use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::{
    analytics_evidence::{EvidenceAccumulator, EvidenceConfig},
    analytics_validation::ValidationSummary,
    backtest::realism::{LatencyModel, QueueModel, RealisticFillModel},
    execution::BinanceEnvironment,
    market::{
        binance::{parse_price_ticks, parse_quantity, BinanceMarketEvent},
        metadata::BinanceDepthSnapshot,
        recorder::market_event_to_json,
        BinanceC2cFxClient, BinanceC2cFxPoller, BinanceMarketConfig, BinanceMarketFeed,
        BinanceMarketStream, FxPollerConfig, FxUpdate, PublicMarketMetadataClient, ReconnectPolicy,
    },
    orderbook::{LocalOrderBook, OrderBookError},
    runtime::{
        io::{spawn_line_writer, write_json_atomic, AsyncLineWriter},
        reference_authority::fetch as load_index_anchor_set,
        DataQuality, EventEnvelope, EventSource,
    },
    simulation_runtime::{
        AnchorSnapshot, PerformancePoint, PositionAllocation, SimulationEngine, SimulationError,
        SimulationPolicyVariant, SimulationSummary,
    },
};

#[derive(Debug, Clone)]
pub struct SimulationBatchSpec {
    pub label: String,
    pub variant: SimulationPolicyVariant,
}

#[derive(Debug, Clone)]
pub struct SimulationBatchConfig {
    /// Human-readable run generation. Each run writes it into its manifest.
    pub policy_id: String,
    pub environment: BinanceEnvironment,
    pub symbols: Vec<String>,
    pub anchors: BTreeMap<String, AnchorSnapshot>,
    pub entry_threshold_bps: i64,
    pub threshold_scale_ppm: i64,
    pub max_position: i64,
    pub requested_quantity: i64,
    pub max_mark_index_gap_bps: i64,
    pub max_anchor_age_ms: u64,
    pub fee_ppm: i64,
    pub quantity_scale: u32,
    pub price_scale: u32,
    pub position_allocations: Option<BTreeMap<String, PositionAllocation>>,
    pub output_root: PathBuf,
    pub specs: Vec<SimulationBatchSpec>,
    pub max_subscriptions_per_shard: usize,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub metrics_refresh_ms: u64,
    pub index_anchor_refresh_ms: u64,
    pub fx_refresh_ms: u64,
    pub fx_max_age_ms: u64,
    pub queue_ahead: i64,
    pub trade_through: i64,
    pub market_to_decision_ms: u64,
    pub decision_to_exchange_ms: u64,
    pub cancel_to_exchange_ms: u64,
    pub quote_reprice_min_interval_ms: u64,
    pub dynamic_capital_refresh_ms: u64,
    /// REST snapshot depth used to seed the live-like local order book.
    pub depth_snapshot_limit: usize,
    pub duration_secs: u64,
    /// Shared, deterministic evidence test fed exactly once per public event.
    pub evidence: EvidenceConfig,
}

#[derive(Debug, Serialize)]
pub struct SimulationLedgerResult {
    pub label: String,
    pub strategy_variant: String,
    pub evidence_record_id: String,
    pub summary: SimulationSummary,
    pub records_written: u64,
    pub records_dropped: u64,
}

#[derive(Debug, Serialize)]
pub struct SimulationBatchResult {
    pub shared_market_records_written: u64,
    pub shared_market_records_dropped: u64,
    pub shared_fx_records_written: u64,
    pub shared_fx_records_dropped: u64,
    pub evidence_summary: crate::analytics_evidence::EvidenceSummary,
    pub analytics_validation_summary: ValidationSummary,
    pub evidence_records_written: u64,
    pub evidence_records_dropped: u64,
    pub ledgers: Vec<SimulationLedgerResult>,
}

struct Ledger {
    spec: SimulationBatchSpec,
    engine: SimulationEngine,
    record_tx: mpsc::Sender<String>,
    record_writer: tokio::task::JoinHandle<Result<u64, std::io::Error>>,
    record_written: Arc<AtomicU64>,
    record_dropped: Arc<AtomicU64>,
    metrics_path: PathBuf,
    history: VecDeque<PerformancePoint>,
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[allow(clippy::type_complexity)]
fn parse_depth_snapshot(
    snapshot: &BinanceDepthSnapshot,
    price_scale: u32,
    quantity_scale: u32,
) -> Result<(u64, Vec<(i64, i64)>, Vec<(i64, i64)>), SimulationError> {
    let parse = |rows: &Vec<[String; 2]>| {
        rows.iter()
            .map(|[price, quantity]| {
                Ok((
                    parse_price_ticks(price, price_scale)
                        .map_err(|error| {
                            SimulationError::Market(format!("invalid depth price: {error:?}"))
                        })?
                        .0,
                    parse_quantity(quantity, quantity_scale)
                        .map_err(|error| {
                            SimulationError::Market(format!("invalid depth quantity: {error:?}"))
                        })?
                        .0,
                ))
            })
            .collect::<Result<Vec<_>, SimulationError>>()
    };
    Ok((
        snapshot.last_update_id,
        parse(&snapshot.bids)?,
        parse(&snapshot.asks)?,
    ))
}

async fn unique_output_root(root: &Path) -> Result<PathBuf, SimulationError> {
    if !tokio::fs::try_exists(root).await? {
        return Ok(root.to_path_buf());
    }
    for run_number in 1..=10_000_u32 {
        let candidate = PathBuf::from(format!("{}-run-{:03}", root.display(), run_number));
        if !tokio::fs::try_exists(&candidate).await? {
            return Ok(candidate);
        }
    }
    Err(SimulationError::InvalidConfig(
        "batch execution output root has too many retained runs",
    ))
}

fn build_engine(
    config: &SimulationBatchConfig,
    spec: &SimulationBatchSpec,
) -> Result<SimulationEngine, SimulationError> {
    let realism = RealisticFillModel {
        queue: QueueModel {
            visible_ahead: config.queue_ahead,
            trade_through: config.trade_through,
        },
        latency: LatencyModel {
            market_to_decision_ms: config.market_to_decision_ms,
            decision_to_exchange_ms: config.decision_to_exchange_ms,
            cancel_to_exchange_ms: config.cancel_to_exchange_ms,
        },
    };
    let mut engine = SimulationEngine::new(
        config.anchors.clone(),
        config.entry_threshold_bps,
        config.max_position,
        config.requested_quantity,
        config.max_mark_index_gap_bps,
        config.max_anchor_age_ms,
        config.fee_ppm,
        config.quantity_scale,
    )?
    .with_price_scale(config.price_scale)
    .with_realism(realism)
    .with_live_risk_gates()
    .with_strategy_variant(spec.variant)
    .with_quote_reprice_min_interval_ms(config.quote_reprice_min_interval_ms)
    .with_dynamic_capital_refresh_ms(config.dynamic_capital_refresh_ms)
    .with_threshold_scale_ppm(config.threshold_scale_ppm);
    if let Some(allocations) = config.position_allocations.clone() {
        engine = engine.with_position_allocations(allocations)?;
    }
    Ok(engine)
}

fn validate(config: &SimulationBatchConfig) -> Result<(), SimulationError> {
    if config.symbols.is_empty()
        || config.specs.is_empty()
        || config.max_subscriptions_per_shard == 0
    {
        return Err(SimulationError::InvalidConfig(
            "batch execution requires symbols, specs, and shard capacity",
        ));
    }
    if config.specs.iter().any(|spec| spec.label.trim().is_empty()) {
        return Err(SimulationError::InvalidConfig(
            "batch execution labels must be non-empty",
        ));
    }
    if config
        .specs
        .windows(2)
        .any(|pair| pair[0].label == pair[1].label)
    {
        return Err(SimulationError::InvalidConfig(
            "batch execution labels must be unique",
        ));
    }
    Ok(())
}
pub async fn run(
    mut config: SimulationBatchConfig,
) -> Result<SimulationBatchResult, SimulationError> {
    validate(&config)?;
    if config.policy_id.trim().is_empty() {
        return Err(SimulationError::InvalidConfig(
            "simulation policy identity must be non-empty",
        ));
    }
    config.output_root = unique_output_root(&config.output_root).await?;
    tokio::fs::create_dir_all(&config.output_root).await?;
    let manifest_created_at_ms = now_ms();
    let parameter_material = serde_json::json!({
        "policy_id": config.policy_id,
        "entry_threshold_bps": config.entry_threshold_bps,
        "threshold_scale_ppm": config.threshold_scale_ppm,
        "fee_ppm": config.fee_ppm,
        "queue_ahead": config.queue_ahead,
        "trade_through": config.trade_through,
        "market_to_decision_ms": config.market_to_decision_ms,
        "decision_to_exchange_ms": config.decision_to_exchange_ms,
        "cancel_to_exchange_ms": config.cancel_to_exchange_ms,
        "dynamic_capital_refresh_ms": config.dynamic_capital_refresh_ms,
        "depth_snapshot_limit": config.depth_snapshot_limit,
        "duration_secs": config.duration_secs,
    });
    let parameter_bytes = serde_json::to_vec(&parameter_material)
        .map_err(|_| SimulationError::InvalidConfig("cannot encode parameter digest"))?;
    let parameter_digest = format!("sha256:{}", hex::encode(Sha256::digest(parameter_bytes)));
    let data_material = serde_json::json!({
        "symbols": config.symbols,
        "anchors": config.anchors.iter().map(|(symbol, anchor)| {
            (symbol, (anchor.close_price_ticks, anchor.observed_at_ms, anchor.valid_until_ms))
        }).collect::<BTreeMap<_, _>>(),
        "environment": config.environment.as_str(),
    });
    let data_bytes = serde_json::to_vec(&data_material)
        .map_err(|_| SimulationError::InvalidConfig("cannot encode data digest"))?;
    let data_digest = format!("sha256:{}", hex::encode(Sha256::digest(data_bytes)));
    let manifest = serde_json::json!({
        "simulation": crate::simulation::SimulationRunManifest::new(
            format!("{}-{}", config.policy_id, manifest_created_at_ms),
            "batch",
            config.policy_id.clone(),
            manifest_created_at_ms,
            config.symbols.clone(),
            config
                .specs
                .iter()
                .map(|spec| spec.variant.label().to_owned())
                .collect(),
        )
        .with_lineage(
            None,
            parameter_digest.clone(),
            data_digest.clone(),
            "isolated",
            None,
        ),
        "policy_id": config.policy_id,
        "created_at_ms": manifest_created_at_ms,
        "parameter_digest": parameter_digest,
        "data_digest": data_digest,
        "strategy_variants": config.specs.iter().map(|spec| spec.variant.label()).collect::<Vec<_>>(),
        "spec_labels": config.specs.iter().map(|spec| spec.label.as_str()).collect::<Vec<_>>(),
        "symbols": config.symbols,
        "output_root": config.output_root,
        "entry_threshold_bps": config.entry_threshold_bps,
        "threshold_scale_ppm": config.threshold_scale_ppm,
        "fee_ppm": config.fee_ppm,
        "queue_ahead": config.queue_ahead,
        "trade_through": config.trade_through,
        "market_to_decision_ms": config.market_to_decision_ms,
        "decision_to_exchange_ms": config.decision_to_exchange_ms,
        "cancel_to_exchange_ms": config.cancel_to_exchange_ms,
        "dynamic_capital_refresh_ms": config.dynamic_capital_refresh_ms,
        "depth_snapshot_limit": config.depth_snapshot_limit,
        "duration_secs": config.duration_secs,
        "evidence": config.evidence.clone(),
    });
    write_json_atomic(&config.output_root.join("run-manifest.json"), &manifest).await?;
    let shared_market_path = config.output_root.join("shared-market.jsonl");
    let shared_fx_path = config.output_root.join("shared-fx.jsonl");
    let AsyncLineWriter {
        sender: market_tx,
        task: market_writer,
        written: market_written,
        dropped: market_dropped,
    } = spawn_line_writer(Some(shared_market_path), 65_536, 1 << 20, 256).await;
    let AsyncLineWriter {
        sender: fx_record_tx,
        task: fx_writer,
        written: fx_written,
        dropped: fx_dropped,
    } = spawn_line_writer(Some(shared_fx_path), 4_096, 1 << 20, 256).await;
    let evidence_path = config.output_root.join("evidence-opportunities.jsonl");
    let AsyncLineWriter {
        sender: evidence_tx,
        task: evidence_writer,
        written: evidence_written,
        dropped: evidence_dropped,
    } = spawn_line_writer(Some(evidence_path), 16_384, 1 << 20, 256).await;
    let mut evidence = EvidenceAccumulator::new(config.evidence.clone());
    let evidence_summary_path = config.output_root.join("evidence-summary.json");
    let analytics_validation_summary = ValidationSummary::default();
    let analytics_validation_path = config.output_root.join("analytics-validation-summary.json");
    write_json_atomic(&analytics_validation_path, &analytics_validation_summary).await?;

    let mut ledgers = Vec::with_capacity(config.specs.len());
    for spec in &config.specs {
        let dir = config.output_root.join(&spec.label);
        tokio::fs::create_dir_all(&dir).await?;
        let AsyncLineWriter {
            sender: record_tx,
            task: record_writer,
            written: record_written,
            dropped: record_dropped,
        } = spawn_line_writer(Some(dir.join("records.jsonl")), 16_384, 1 << 20, 256).await;
        ledgers.push(Ledger {
            spec: spec.clone(),
            engine: build_engine(&config, spec)?,
            record_tx,
            record_writer,
            record_written,
            record_dropped,
            metrics_path: dir.join("metrics.json"),
            history: VecDeque::with_capacity(900),
        });
    }

    let fx_currencies = config
        .symbols
        .iter()
        .filter_map(|symbol| crate::strategy::profile_for(symbol).map(|p| p.anchor_currency))
        .collect::<Vec<_>>();
    let mut unique_fx = Vec::new();
    for currency in fx_currencies {
        if !unique_fx.contains(&currency) {
            unique_fx.push(currency);
        }
    }
    let fx_client = BinanceC2cFxClient::new(None)
        .map_err(|error| SimulationError::Market(format!("FX client: {error}")))?;
    let fx_poller = BinanceC2cFxPoller::new(
        fx_client,
        &unique_fx,
        FxPollerConfig {
            refresh_interval_ms: config.fx_refresh_ms,
            max_stale_ms: config.fx_max_age_ms,
            max_backoff_ms: FxPollerConfig::high_frequency().max_backoff_ms,
        },
    )
    .map_err(|error| SimulationError::Market(format!("FX poller: {error}")))?;
    let (fx_tx, mut fx_rx) = mpsc::channel::<FxUpdate>(256);
    let mut fx_task = tokio::spawn(fx_poller.run(fx_tx));
    let (anchor_tx, mut anchor_rx) = mpsc::channel(1);
    let mut anchor_task = if config.index_anchor_refresh_ms > 0 {
        let environment = config.environment;
        let symbols = config.symbols.clone();
        let refresh_ms = config.index_anchor_refresh_ms;
        Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(refresh_ms.max(1_000))).await;
                if let Ok(anchor_set) = load_index_anchor_set(environment, &symbols, 8, None).await
                {
                    if anchor_tx.send(anchor_set.anchors).await.is_err() {
                        break;
                    }
                }
            }
        }))
    } else {
        None
    };

    let mut shard_tasks = tokio::task::JoinSet::new();
    let (event_tx, mut event_rx) = mpsc::channel::<BinanceMarketEvent>(65_536);
    let event_dropped = Arc::new(AtomicU64::new(0));
    let endpoints = config.environment.endpoints();

    // Start buffering diff-depth before the REST snapshot, matching Binance's
    // required bootstrap order and preserving events during snapshot latency.
    let depth_configs = BinanceMarketConfig::for_symbols(
        endpoints.public_market_ws_base,
        &config.symbols,
        BinanceMarketFeed::OrderBookDepth,
        config.price_scale,
        config.quantity_scale,
        1_048_576,
        config.connect_timeout_ms,
        config.read_timeout_ms,
        None,
        ReconnectPolicy::default(),
        config.max_subscriptions_per_shard,
    )
    .map_err(|error| SimulationError::Market(error.to_string()))?;
    for stream_config in depth_configs {
        let tx = event_tx.clone();
        let dropped = Arc::clone(&event_dropped);
        shard_tasks.spawn(async move {
            BinanceMarketStream::run_forever(stream_config, |event| {
                if tx.try_send(event).is_err() {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            })
            .await;
        });
    }

    let depth_client = PublicMarketMetadataClient::new(endpoints.rest_base, None)
        .map_err(|error| SimulationError::Market(format!("depth snapshot client: {error}")))?;
    let mut depth_books = BTreeMap::<String, LocalOrderBook>::new();
    for symbol in &config.symbols {
        let snapshot = loop {
            match depth_client
                .depth_snapshot(symbol, config.depth_snapshot_limit)
                .await
            {
                Ok(snapshot) => break snapshot,
                Err(error)
                    if {
                        let message = error.to_string();
                        message.contains("429")
                            || message.contains("418")
                            || message.contains("transport failed")
                            || message.contains("timed out")
                    } =>
                {
                    let retry_at = now_ms().saturating_add(60_000);
                    eprintln!(
                        "depth bootstrap rate-limited for {symbol}; retrying after {retry_at}"
                    );
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                Err(error) => {
                    return Err(SimulationError::Market(format!(
                        "depth snapshot {symbol}: {error}"
                    )));
                }
            }
        };
        let mut book = LocalOrderBook::default();
        let bids = snapshot
            .bids
            .iter()
            .map(|[price, quantity]| {
                Ok((
                    parse_price_ticks(price, config.price_scale)
                        .map_err(|error| format!("invalid bid price: {error:?}"))?
                        .0,
                    parse_quantity(quantity, config.quantity_scale)
                        .map_err(|error| format!("invalid bid quantity: {error:?}"))?
                        .0,
                ))
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(|error| {
                SimulationError::Market(format!("depth snapshot {symbol}: {error}"))
            })?;
        let asks = snapshot
            .asks
            .iter()
            .map(|[price, quantity]| {
                Ok((
                    parse_price_ticks(price, config.price_scale)
                        .map_err(|error| format!("invalid ask price: {error:?}"))?
                        .0,
                    parse_quantity(quantity, config.quantity_scale)
                        .map_err(|error| format!("invalid ask quantity: {error:?}"))?
                        .0,
                ))
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(|error| {
                SimulationError::Market(format!("depth snapshot {symbol}: {error}"))
            })?;
        book.load_snapshot(snapshot.last_update_id, &bids, &asks)
            .map_err(|error| {
                SimulationError::Market(format!("depth snapshot {symbol}: {error:?}"))
            })?;
        for ledger in &mut ledgers {
            ledger
                .engine
                .load_depth_snapshot(symbol, snapshot.last_update_id, &bids, &asks)?;
        }
        depth_books.insert(symbol.to_ascii_uppercase(), book);
    }
    let mut next_depth_resync_at_ms = BTreeMap::<String, u64>::new();
    let mut shard_configs = BinanceMarketConfig::for_symbols(
        endpoints.public_market_ws_base,
        &config.symbols,
        BinanceMarketFeed::BookTicker,
        config.price_scale,
        config.quantity_scale,
        1_048_576,
        config.connect_timeout_ms,
        config.read_timeout_ms,
        None,
        ReconnectPolicy::default(),
        config.max_subscriptions_per_shard,
    )
    .map_err(|error| SimulationError::Market(error.to_string()))?;
    shard_configs.extend(
        BinanceMarketConfig::for_symbols(
            endpoints.market_ws_base,
            &config.symbols,
            BinanceMarketFeed::ReferenceAndTrades,
            config.price_scale,
            config.quantity_scale,
            1_048_576,
            config.connect_timeout_ms,
            config.read_timeout_ms,
            None,
            ReconnectPolicy::default(),
            config.max_subscriptions_per_shard,
        )
        .map_err(|error| SimulationError::Market(error.to_string()))?,
    );
    for stream_config in shard_configs {
        let tx = event_tx.clone();
        let dropped = Arc::clone(&event_dropped);
        shard_tasks.spawn(async move {
            BinanceMarketStream::run_forever(stream_config, |event| {
                if tx.try_send(event).is_err() {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            })
            .await;
        });
    }
    drop(event_tx);

    let run_duration = if config.duration_secs == 0 {
        Duration::from_secs(10_000 * 365 * 24 * 60 * 60)
    } else {
        Duration::from_secs(config.duration_secs)
    };
    let mut metrics_interval =
        tokio::time::interval(Duration::from_millis(config.metrics_refresh_ms.max(250)));
    let mut last_received_at_ms = 0_u64;
    let mut event_sequence = 0_u64;
    let mut fx_latest = BTreeMap::<String, FxUpdate>::new();
    let run_result = tokio::time::timeout(run_duration, async {
        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    let Some(event) = event else { return Err::<(), SimulationError>(SimulationError::Market("all market shards stopped".to_owned())); };
                    let received_at = now_ms();
                    last_received_at_ms = received_at;
                    if let BinanceMarketEvent::DepthUpdate(depth) = &event {
                        let symbol = depth.symbol.to_ascii_uppercase();
                        let resync = {
                            let book = depth_books.get_mut(&symbol).ok_or_else(|| {
                                SimulationError::Market(format!("depth update for unknown symbol {symbol}"))
                            })?;
                            match book.apply_diff(depth) {
                                Ok(_) => false,
                                Err(OrderBookError::SequenceGap { .. })
                                | Err(OrderBookError::SnapshotRequired) => true,
                                Err(error) => {
                                    return Err(SimulationError::Market(format!(
                                        "depth book invalid for {symbol}: {error:?}"
                                    )));
                                }
                            }
                        };
                        if resync {
                            if next_depth_resync_at_ms
                                .get(&symbol)
                                .copied()
                                .is_some_and(|deadline| received_at < deadline)
                            {
                                continue;
                            }
                            let snapshot = match depth_client
                                .depth_snapshot(&symbol, config.depth_snapshot_limit)
                                .await
                            {
                                Ok(snapshot) => snapshot,
                                Err(error)
                                    if error.to_string().contains("429")
                                        || error.to_string().contains("418") => {
                                    let retry_at = received_at.saturating_add(60_000);
                                    next_depth_resync_at_ms.insert(symbol.clone(), retry_at);
                                    eprintln!(
                                        "depth resync rate-limited for {symbol}; retrying after {retry_at}"
                                    );
                                    continue;
                                }
                                Err(error) => {
                                    return Err(SimulationError::Market(format!(
                                        "depth resync snapshot {symbol}: {error}"
                                    )));
                                }
                            };
                            let (last_update_id, bids, asks) = parse_depth_snapshot(
                                &snapshot,
                                config.price_scale,
                                config.quantity_scale,
                            )?;
                            depth_books
                                .get_mut(&symbol)
                                .ok_or_else(|| {
                                    SimulationError::Market(format!(
                                        "depth resync book disappeared for {symbol}"
                                    ))
                                })?
                                .load_snapshot(last_update_id, &bids, &asks)
                                .map_err(|error| {
                                    SimulationError::Market(format!(
                                        "depth resync invalid for {symbol}: {error:?}"
                                    ))
                                })?;
                            for ledger in &mut ledgers {
                                ledger.engine.load_depth_snapshot(
                                    &symbol,
                                    last_update_id,
                                    &bids,
                                    &asks,
                                )?;
                            }
                            next_depth_resync_at_ms.remove(&symbol);
                            continue;
                        }
                    }
                    event_sequence = event_sequence.saturating_add(1);
                    let envelope = EventEnvelope {
                        event_id: format!("market-{event_sequence}").into(),
                        run_id: format!("batch-{}", config.policy_id).into(),
                        causality_id: format!("market-cause-{event_sequence}").into(),
                        source: EventSource::BinancePublic,
                        observed_at_ms: crate::simulation_runtime::event_time_ms(&event),
                        received_at_ms: received_at,
                        sequence: event_sequence,
                        state_version: event_sequence,
                        quality: DataQuality::Trusted,
                        payload: event.clone(),
                    };
                    let market_line = serde_json::to_string(&market_event_to_json(&event, config.price_scale, config.quantity_scale, Some(received_at)))?;
                    if market_tx.try_send(market_line).is_err() { market_dropped.fetch_add(1, Ordering::Relaxed); }
                    for evidence in evidence.observe(&event, received_at, &config.anchors) {
                        let line = serde_json::to_string(&evidence)?;
                        if evidence_tx.try_send(line).is_err() { evidence_dropped.fetch_add(1, Ordering::Relaxed); }
                    }
                    for ledger in &mut ledgers {
                        for record in ledger.engine.on_enveloped_event(&envelope)? {
                            let line = serde_json::to_string(&record)?;
                            if ledger.record_tx.try_send(line).is_err() { ledger.record_dropped.fetch_add(1, Ordering::Relaxed); }
                        }
                    }
                }
                _ = metrics_interval.tick() => {
                    let observed_at = now_ms();
                    write_json_atomic(&evidence_summary_path, &evidence.summary()).await?;
                    for ledger in &mut ledgers {
                        ledger.history.push_back(ledger.engine.performance_point(observed_at));
                        while ledger.history.len() > 900 { ledger.history.pop_front(); }
                        let snapshot = ledger.engine.metrics_snapshot_with_history(observed_at, last_received_at_ms, ledger.history.make_contiguous());
                        write_json_atomic(&ledger.metrics_path, &snapshot).await?;
                    }
                }
                anchor_update = anchor_rx.recv(), if config.index_anchor_refresh_ms > 0 => {
                    if let Some(anchors) = anchor_update {
                        let timestamp = now_ms();
                        config.anchors = anchors.clone();
                        for ledger in &mut ledgers {
                            ledger.engine.refresh_anchors(anchors.clone(), timestamp);
                        }
                    }
                }
                update = fx_rx.recv() => {
                    let Some(update) = update else { return Err::<(), SimulationError>(SimulationError::Market("FX feed stopped".to_owned())); };
                    fx_latest.insert(update.currency.clone(), update.clone());
                    let line = serde_json::to_string(&update)?;
                    if fx_record_tx.try_send(line).is_err() { fx_dropped.fetch_add(1, Ordering::Relaxed); }
                }
                joined = shard_tasks.join_next() => {
                    match joined {
                        Some(Ok(())) => {
                            eprintln!("market shard supervisor ended unexpectedly; waiting for feed recovery");
                        }
                        Some(Err(error)) => {
                            return Err(SimulationError::Market(format!(
                                "market shard task failed: {error}"
                            )));
                        }
                        None => {
                            return Err(SimulationError::Market(
                                "all market shard supervisors stopped".to_owned(),
                            ));
                        }
                    }
                }
                fx_joined = &mut fx_task => {
                    return match fx_joined {
                        Ok(Ok(())) => Err(SimulationError::Market("FX feed stopped".to_owned())),
                        Ok(Err(error)) => Err(SimulationError::Market(format!("FX feed failed: {error}"))),
                        Err(error) => Err(SimulationError::Market(format!("FX task failed: {error}"))),
                    };
                }
            }
        }
    }).await;
    let run_error = match run_result {
        Ok(Err(error)) => Some(error),
        Err(_) if config.duration_secs == 0 => Some(SimulationError::Market(
            "continuous batch execution timeout".to_owned(),
        )),
        _ => None,
    };

    shard_tasks.abort_all();
    while shard_tasks.join_next().await.is_some() {}
    fx_task.abort();
    let _ = fx_task.await;
    if let Some(anchor_task) = anchor_task.take() {
        anchor_task.abort();
        let _ = anchor_task.await;
    }
    for ledger in &mut ledgers {
        for record in ledger
            .engine
            .cancel_all(now_ms(), "batch execution stopped")
        {
            let line = serde_json::to_string(&record)?;
            if ledger.record_tx.try_send(line).is_err() {
                ledger.record_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        let observed_at = now_ms();
        ledger
            .history
            .push_back(ledger.engine.performance_point(observed_at));
        while ledger.history.len() > 900 {
            ledger.history.pop_front();
        }
        let snapshot = ledger.engine.metrics_snapshot_with_history(
            observed_at,
            last_received_at_ms,
            ledger.history.make_contiguous(),
        );
        write_json_atomic(&ledger.metrics_path, &snapshot).await?;
    }
    write_json_atomic(&evidence_summary_path, &evidence.summary()).await?;
    drop(market_tx);
    drop(fx_record_tx);
    drop(evidence_tx);
    let market_count = market_writer
        .await
        .map_err(|e| SimulationError::Io(e.to_string()))??;
    let fx_count = fx_writer
        .await
        .map_err(|e| SimulationError::Io(e.to_string()))??;
    let evidence_count = evidence_writer
        .await
        .map_err(|e| SimulationError::Io(e.to_string()))??;
    let evidence_summary = evidence.summary();
    let mut ledger_results = Vec::with_capacity(ledgers.len());
    for ledger in ledgers {
        drop(ledger.record_tx);
        let count = ledger
            .record_writer
            .await
            .map_err(|e| SimulationError::Io(e.to_string()))??;
        ledger_results.push(SimulationLedgerResult {
            label: ledger.spec.label,
            strategy_variant: ledger.spec.variant.label().to_owned(),
            evidence_record_id: evidence.evidence_id(),
            summary: ledger.engine.summary(),
            records_written: count.max(ledger.record_written.load(Ordering::Relaxed)),
            records_dropped: ledger.record_dropped.load(Ordering::Relaxed),
        });
    }
    if let Some(error) = run_error {
        return Err(error);
    }
    if event_dropped.load(Ordering::Relaxed) != 0
        || market_dropped.load(Ordering::Relaxed) != 0
        || fx_dropped.load(Ordering::Relaxed) != 0
        || evidence_dropped.load(Ordering::Relaxed) != 0
    {
        return Err(SimulationError::Market(
            "batch execution dropped shared feed records".to_owned(),
        ));
    }
    Ok(SimulationBatchResult {
        shared_market_records_written: market_count.max(market_written.load(Ordering::Relaxed)),
        shared_market_records_dropped: market_dropped.load(Ordering::Relaxed),
        shared_fx_records_written: fx_count.max(fx_written.load(Ordering::Relaxed)),
        shared_fx_records_dropped: fx_dropped.load(Ordering::Relaxed),
        evidence_summary,
        analytics_validation_summary,
        evidence_records_written: evidence_count.max(evidence_written.load(Ordering::Relaxed)),
        evidence_records_dropped: evidence_dropped.load(Ordering::Relaxed),
        ledgers: ledger_results,
    })
}
