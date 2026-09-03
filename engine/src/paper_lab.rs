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
use tokio::{io::AsyncWriteExt, sync::mpsc};

use crate::{
    backtest::realism::{LatencyModel, QueueModel, RealisticFillModel},
    execution::BinanceEnvironment,
    market::{
        binance::BinanceMarketEvent, BinanceC2cFxClient, BinanceC2cFxPoller, BinanceMarketConfig,
        BinanceMarketStream, BinanceSubscription, FxPollerConfig, FxUpdate, ReconnectPolicy,
    },
    paper::{
        load_binance_index_anchor_set, market_event_to_json, PaperAnchor, PaperEngine, PaperError,
        PaperMetricsSnapshot, PaperPerformancePoint, PaperStrategyVariant, PositionAllocation,
    },
};

#[derive(Debug, Clone)]
pub struct PaperLabSpec {
    pub label: String,
    pub variant: PaperStrategyVariant,
}

#[derive(Debug, Clone)]
pub struct PaperLabConfig {
    pub environment: BinanceEnvironment,
    pub symbols: Vec<String>,
    pub anchors: BTreeMap<String, PaperAnchor>,
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
    pub specs: Vec<PaperLabSpec>,
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
    pub duration_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct PaperLabLedgerResult {
    pub label: String,
    pub strategy_variant: String,
    pub summary: crate::paper::PaperSummary,
    pub records_written: u64,
    pub records_dropped: u64,
}

#[derive(Debug, Serialize)]
pub struct PaperLabResult {
    pub shared_market_records_written: u64,
    pub shared_market_records_dropped: u64,
    pub shared_fx_records_written: u64,
    pub shared_fx_records_dropped: u64,
    pub ledgers: Vec<PaperLabLedgerResult>,
}

struct Ledger {
    spec: PaperLabSpec,
    engine: PaperEngine,
    record_tx: mpsc::Sender<String>,
    record_writer: tokio::task::JoinHandle<Result<u64, PaperError>>,
    record_written: Arc<AtomicU64>,
    record_dropped: Arc<AtomicU64>,
    metrics_path: PathBuf,
    history: VecDeque<PaperPerformancePoint>,
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn stream_config(
    environment: BinanceEnvironment,
    symbols: &[String],
    public: bool,
    price_scale: u32,
    quantity_scale: u32,
    max_subscriptions_per_shard: usize,
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
) -> Result<Vec<BinanceMarketConfig>, PaperError> {
    let subscriptions = symbols
        .iter()
        .map(|symbol| {
            let subscription = BinanceSubscription::new(symbol)
                .map_err(|_| PaperError::InvalidConfig("invalid market symbol"))?;
            Ok(if public {
                subscription.book_ticker_only()
            } else {
                subscription.market_reference_and_trades()
            })
        })
        .collect::<Result<Vec<_>, PaperError>>()?;
    let endpoints = environment.endpoints();
    let config = BinanceMarketConfig {
        market_ws_base: if public {
            endpoints.public_market_ws_base.into()
        } else {
            endpoints.market_ws_base.into()
        },
        subscriptions,
        price_scale,
        quantity_scale,
        max_frame_bytes: 1_048_576,
        connect_timeout_ms,
        read_timeout_ms,
        http_proxy: None,
        reconnect: ReconnectPolicy::default(),
    };
    config
        .into_shards(max_subscriptions_per_shard)
        .map_err(|error| PaperError::Market(error.to_string()))
}

async fn spawn_line_writer(
    path: PathBuf,
    capacity: usize,
) -> Result<
    (
        mpsc::Sender<String>,
        tokio::task::JoinHandle<Result<u64, PaperError>>,
        Arc<AtomicU64>,
        Arc<AtomicU64>,
    ),
    PaperError,
> {
    let (tx, mut rx) = mpsc::channel::<String>(capacity);
    let written = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let written_count = Arc::clone(&written);
    let task = tokio::spawn(async move {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = tokio::fs::File::create(path).await?;
        let mut file = tokio::io::BufWriter::with_capacity(1 << 20, file);
        let mut pending = 0_u32;
        while let Some(line) = rx.recv().await {
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
            written_count.fetch_add(1, Ordering::Relaxed);
            pending += 1;
            if pending >= 256 {
                file.flush().await?;
                pending = 0;
            }
        }
        file.flush().await?;
        Ok(written_count.load(Ordering::Relaxed))
    });
    Ok((tx, task, written, dropped))
}
async fn write_metrics(path: &Path, snapshot: &PaperMetricsSnapshot) -> Result<(), PaperError> {
    let bytes = serde_json::to_vec(snapshot)?;
    let temporary = path.with_extension("json.tmp");
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(&temporary, path).await?;
    Ok(())
}

fn build_engine(config: &PaperLabConfig, spec: &PaperLabSpec) -> Result<PaperEngine, PaperError> {
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
    let mut engine = PaperEngine::new(
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
    .with_threshold_scale_ppm(config.threshold_scale_ppm);
    if let Some(allocations) = config.position_allocations.clone() {
        engine = engine.with_position_allocations(allocations)?;
    }
    Ok(engine)
}

fn validate(config: &PaperLabConfig) -> Result<(), PaperError> {
    if config.symbols.is_empty()
        || config.specs.is_empty()
        || config.max_subscriptions_per_shard == 0
    {
        return Err(PaperError::InvalidConfig(
            "paper lab requires symbols, specs, and shard capacity",
        ));
    }
    if config.specs.iter().any(|spec| spec.label.trim().is_empty()) {
        return Err(PaperError::InvalidConfig(
            "paper lab labels must be non-empty",
        ));
    }
    if config
        .specs
        .windows(2)
        .any(|pair| pair[0].label == pair[1].label)
    {
        return Err(PaperError::InvalidConfig("paper lab labels must be unique"));
    }
    Ok(())
}
pub async fn run(config: PaperLabConfig) -> Result<PaperLabResult, PaperError> {
    validate(&config)?;
    tokio::fs::create_dir_all(&config.output_root).await?;
    let shared_market_path = config.output_root.join("shared-market.jsonl");
    let shared_fx_path = config.output_root.join("shared-fx.jsonl");
    let (market_tx, market_writer, market_written, market_dropped) =
        spawn_line_writer(shared_market_path, 65_536).await?;
    let (fx_record_tx, fx_writer, fx_written, fx_dropped) =
        spawn_line_writer(shared_fx_path, 4_096).await?;

    let mut ledgers = Vec::with_capacity(config.specs.len());
    for spec in &config.specs {
        let dir = config.output_root.join(&spec.label);
        tokio::fs::create_dir_all(&dir).await?;
        let (record_tx, record_writer, record_written, record_dropped) =
            spawn_line_writer(dir.join("records.jsonl"), 16_384).await?;
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
        .map_err(|error| PaperError::Market(format!("FX client: {error}")))?;
    let fx_poller = BinanceC2cFxPoller::new(
        fx_client,
        &unique_fx,
        FxPollerConfig {
            refresh_interval_ms: config.fx_refresh_ms,
            max_stale_ms: config.fx_max_age_ms,
            max_backoff_ms: 30_000,
        },
    )
    .map_err(|error| PaperError::Market(format!("FX poller: {error}")))?;
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
                if let Ok(anchor_set) =
                    load_binance_index_anchor_set(environment, &symbols, 8, None).await
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
    for stream_config in stream_config(
        config.environment,
        &config.symbols,
        true,
        config.price_scale,
        config.quantity_scale,
        config.max_subscriptions_per_shard,
        config.connect_timeout_ms,
        config.read_timeout_ms,
    )?
    .into_iter()
    .chain(stream_config(
        config.environment,
        &config.symbols,
        false,
        config.price_scale,
        config.quantity_scale,
        config.max_subscriptions_per_shard,
        config.connect_timeout_ms,
        config.read_timeout_ms,
    )?) {
        let tx = event_tx.clone();
        let dropped = Arc::clone(&event_dropped);
        shard_tasks.spawn(async move {
            let mut stream = BinanceMarketStream::new(stream_config);
            stream
                .run_until_error(|event| {
                    if tx.try_send(event).is_err() {
                        dropped.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .await
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
    let mut fx_latest = BTreeMap::<String, FxUpdate>::new();
    let run_result = tokio::time::timeout(run_duration, async {
        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    let Some(event) = event else { return Err::<(), PaperError>(PaperError::Market("all market shards stopped".to_owned())); };
                    let received_at = now_ms();
                    last_received_at_ms = received_at;
                    let market_line = serde_json::to_string(&market_event_to_json(&event, config.price_scale, config.quantity_scale, Some(received_at)))?;
                    if market_tx.try_send(market_line).is_err() { market_dropped.fetch_add(1, Ordering::Relaxed); }
                    for ledger in &mut ledgers {
                        for record in ledger.engine.on_event_ref(&event) {
                            let line = serde_json::to_string(&record)?;
                            if ledger.record_tx.try_send(line).is_err() { ledger.record_dropped.fetch_add(1, Ordering::Relaxed); }
                        }
                    }
                }
                _ = metrics_interval.tick() => {
                    let observed_at = now_ms();
                    for ledger in &mut ledgers {
                        ledger.history.push_back(ledger.engine.performance_point(observed_at));
                        while ledger.history.len() > 900 { ledger.history.pop_front(); }
                        let snapshot = ledger.engine.metrics_snapshot_with_history(observed_at, last_received_at_ms, ledger.history.make_contiguous());
                        write_metrics(&ledger.metrics_path, &snapshot).await?;
                    }
                }
                anchor_update = anchor_rx.recv(), if config.index_anchor_refresh_ms > 0 => {
                    if let Some(anchors) = anchor_update {
                        let timestamp = now_ms();
                        for ledger in &mut ledgers {
                            ledger.engine.refresh_anchors(anchors.clone(), timestamp);
                        }
                    }
                }
                update = fx_rx.recv() => {
                    let Some(update) = update else { return Err::<(), PaperError>(PaperError::Market("FX feed stopped".to_owned())); };
                    fx_latest.insert(update.currency.clone(), update.clone());
                    let line = serde_json::to_string(&update)?;
                    if fx_record_tx.try_send(line).is_err() { fx_dropped.fetch_add(1, Ordering::Relaxed); }
                }
                joined = shard_tasks.join_next() => {
                    return match joined {
                        Some(Ok(Ok(()))) => Err(PaperError::Market("market shard stopped".to_owned())),
                        Some(Ok(Err(error))) => Err(PaperError::Market(error.to_string())),
                        Some(Err(error)) => Err(PaperError::Market(format!("market shard task failed: {error}"))),
                        None => Err(PaperError::Market("all market shards stopped".to_owned())),
                    };
                }
                fx_joined = &mut fx_task => {
                    return match fx_joined {
                        Ok(Ok(())) => Err(PaperError::Market("FX feed stopped".to_owned())),
                        Ok(Err(error)) => Err(PaperError::Market(format!("FX feed failed: {error}"))),
                        Err(error) => Err(PaperError::Market(format!("FX task failed: {error}"))),
                    };
                }
            }
        }
    }).await;
    let run_error = match run_result {
        Ok(Err(error)) => Some(error),
        Err(_) if config.duration_secs == 0 => Some(PaperError::Market(
            "continuous paper lab timeout".to_owned(),
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
        for record in ledger.engine.cancel_all(now_ms(), "paper lab stopped") {
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
        write_metrics(&ledger.metrics_path, &snapshot).await?;
    }
    drop(market_tx);
    drop(fx_record_tx);
    let market_count = market_writer
        .await
        .map_err(|e| PaperError::Io(e.to_string()))??;
    let fx_count = fx_writer
        .await
        .map_err(|e| PaperError::Io(e.to_string()))??;
    let mut ledger_results = Vec::with_capacity(ledgers.len());
    for ledger in ledgers {
        drop(ledger.record_tx);
        let count = ledger
            .record_writer
            .await
            .map_err(|e| PaperError::Io(e.to_string()))??;
        ledger_results.push(PaperLabLedgerResult {
            label: ledger.spec.label,
            strategy_variant: ledger.spec.variant.label().to_owned(),
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
    {
        return Err(PaperError::Market(
            "paper lab dropped shared feed records".to_owned(),
        ));
    }
    Ok(PaperLabResult {
        shared_market_records_written: market_count.max(market_written.load(Ordering::Relaxed)),
        shared_market_records_dropped: market_dropped.load(Ordering::Relaxed),
        shared_fx_records_written: fx_count.max(fx_written.load(Ordering::Relaxed)),
        shared_fx_records_dropped: fx_dropped.load(Ordering::Relaxed),
        ledgers: ledger_results,
    })
}
