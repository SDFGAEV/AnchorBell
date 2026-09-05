use std::{
    collections::BTreeMap,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    process,
    str::FromStr,
    time::Duration,
};

use static_anchor_engine::{
    execution::BinanceEnvironment,
    hypothesis::HypothesisConfig,
    paper::{
        allocate_positions, load_binance_index_anchor_set, PaperStrategyVariant, PositionMode,
    },
    paper_lab::{run, PaperLabConfig, PaperLabSpec},
};

const DEFAULT_SYMBOLS: &str =
    "CXMTUSDT,UNITREEUSDT,GIGADEVUSDT,HK0625USDT,MINIMAXUSDT,ZHIPUUSDT,ZHONGJIUSDT";

#[derive(Debug)]
struct Args {
    experiment_version: String,
    environment: BinanceEnvironment,
    anchors: Option<PathBuf>,
    index_anchors: bool,
    symbols: Vec<String>,
    output_root: PathBuf,
    capital_usdt: i64,
    entry_threshold_bps: i64,
    threshold_scale_ppm: i64,
    max_mark_index_gap_bps: i64,
    fee_ppm: i64,
    queue_ahead: i64,
    trade_through: i64,
    market_to_decision_ms: u64,
    decision_to_exchange_ms: u64,
    cancel_to_exchange_ms: u64,
    quote_reprice_min_interval_ms: u64,
    dynamic_capital_refresh_ms: u64,
    duration_secs: u64,
}

fn main() {
    let args = parse_args().unwrap_or_else(|error| fail(error));
    if args.anchors.is_some() {
        fail(
            "paper lab forbids local --anchors; use live --index-anchors so the immutable anchor is fetched from Binance at startup",
        );
    }
    if !args.index_anchors {
        fail("paper lab requires live --index-anchors");
    }
    let _instance_guard = claim_single_paper_lab_instance().unwrap_or_else(|error| fail(error));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| fail(format!("cannot create runtime: {error}")));
    runtime.block_on(async move {
        // Never reuse a local anchor for a live paper run. Bootstrap must obtain
        // the current Binance index/FX-derived anchor set before any market
        // event is admitted; transient REST failures wait and retry.
        let anchors = loop {
            match load_binance_index_anchor_set(args.environment, &args.symbols, 8, None).await {
                Ok(set) => break set.anchors,
                Err(error)
                    if {
                        let message = error.to_string();
                        message.contains("429")
                            || message.contains("418")
                            || message.contains("transport failed")
                            || message.contains("timed out")
                            || message.contains("stale or has a future")
                    } =>
                {
                    eprintln!("index anchor bootstrap transient failure: {error}; retrying in 60s");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
                Err(error) => fail(format!("cannot load paper-lab index anchors: {error}")),
            }
        };
        let anchors = anchors
            .into_iter()
            .filter(|(symbol, _)| {
                args.symbols
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(symbol))
            })
            .collect::<BTreeMap<_, _>>();
        let modes = BTreeMap::<String, PositionMode>::new();
        let allocations = allocate_positions(&anchors, args.capital_usdt, &modes, 8)
            .unwrap_or_else(|error| fail(format!("cannot allocate paper-lab capital: {error}")));
        let specs = vec![
            ("F1_m1", PaperStrategyVariant::M1AdaptiveRisk),
            ("F2_m2", PaperStrategyVariant::M2Microstructure),
            ("F3_m3", PaperStrategyVariant::M3FillAware),
            ("F4_m4", PaperStrategyVariant::M4Statistical),
            ("F5_m5", PaperStrategyVariant::M5Robust),
            ("F6_m6", PaperStrategyVariant::M6DynamicCapital),
            ("F7_m7", PaperStrategyVariant::M7EvidenceGated),
            ("M8_full", PaperStrategyVariant::M8FundingAware),
            ("M8_no_funding", PaperStrategyVariant::M7EvidenceGated),
            ("R7_m7", PaperStrategyVariant::M7EvidenceGated),
            ("R6_m6", PaperStrategyVariant::M6DynamicCapital),
            ("R5_m5", PaperStrategyVariant::M5Robust),
            ("R4_m4", PaperStrategyVariant::M4Statistical),
            ("R3_m3", PaperStrategyVariant::M3FillAware),
            ("R2_m2", PaperStrategyVariant::M2Microstructure),
            ("R1_m1", PaperStrategyVariant::M1AdaptiveRisk),
        ]
        .into_iter()
        .map(|(label, variant)| PaperLabSpec {
            label: label.to_owned(),
            variant,
        })
        .collect();
        let config = PaperLabConfig {
            experiment_version: args.experiment_version,
            environment: args.environment,
            symbols: args.symbols,
            anchors,
            entry_threshold_bps: args.entry_threshold_bps,
            threshold_scale_ppm: args.threshold_scale_ppm,
            max_position: 10_000_000,
            requested_quantity: 1_000_000,
            max_mark_index_gap_bps: args.max_mark_index_gap_bps,
            max_anchor_age_ms: 0,
            fee_ppm: args.fee_ppm,
            quantity_scale: 8,
            price_scale: 8,
            position_allocations: Some(allocations),
            output_root: args.output_root,
            specs,
            max_subscriptions_per_shard: 64,
            connect_timeout_ms: 5_000,
            read_timeout_ms: 15_000,
            metrics_refresh_ms: 1_000,
            index_anchor_refresh_ms: if args.index_anchors { 60_000 } else { 0 },
            fx_refresh_ms: 30_000,
            fx_max_age_ms: 120_000,
            queue_ahead: args.queue_ahead,
            trade_through: args.trade_through,
            market_to_decision_ms: args.market_to_decision_ms,
            decision_to_exchange_ms: args.decision_to_exchange_ms,
            cancel_to_exchange_ms: args.cancel_to_exchange_ms,
            quote_reprice_min_interval_ms: args.quote_reprice_min_interval_ms,
            dynamic_capital_refresh_ms: args.dynamic_capital_refresh_ms,
            // Keep REST weight bounded; resync is throttled on 418/429.
            depth_snapshot_limit: 100,
            duration_secs: args.duration_secs,
            hypothesis: HypothesisConfig::default(),
        };
        let result = run(config)
            .await
            .unwrap_or_else(|error| fail(format!("paper lab failed: {error}")));
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("lab result is serializable")
        );
    });
}
fn parse_args() -> Result<Args, String> {
    let mut experiment_version = "M7-ablation-r13".to_owned();
    let mut environment = BinanceEnvironment::Production;
    let mut anchors = None;
    let mut index_anchors = true;
    let mut symbols = DEFAULT_SYMBOLS
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut output_root = PathBuf::from("target\\paper-lab-20260904-M7");
    let mut capital_usdt = 1_500_i64.checked_mul(100_000_000).unwrap();
    let mut entry_threshold_bps = 5;
    let mut threshold_scale_ppm = 700_000;
    let mut max_mark_index_gap_bps = 50;
    let mut fee_ppm = 200;
    let mut queue_ahead = 0;
    let mut trade_through = 0;
    let mut market_to_decision_ms = 0;
    let mut decision_to_exchange_ms = 0;
    let mut cancel_to_exchange_ms = 0;
    let mut quote_reprice_min_interval_ms = 750;
    let mut dynamic_capital_refresh_ms = 60_000;
    let mut duration_secs = 0;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--experiment-version" => experiment_version = next(&mut args, &flag)?,
            "--anchors" => {
                anchors = Some(PathBuf::from(next(&mut args, &flag)?));
                index_anchors = false;
            }
            "--index-anchors" => index_anchors = true,
            "--environment" => {
                environment = next(&mut args, &flag)?
                    .parse()
                    .map_err(|_| "invalid --environment".to_owned())?;
            }
            "--symbols" => {
                symbols = next(&mut args, &flag)?
                    .split(',')
                    .map(|s| s.trim().to_ascii_uppercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            "--output-root" => output_root = PathBuf::from(next(&mut args, &flag)?),
            "--capital-usdt" => capital_usdt = parse_decimal(&next(&mut args, &flag)?, 8)?,
            "--entry-threshold-bps" => entry_threshold_bps = parse(&mut args, &flag)?,
            "--threshold-scale-ppm" => threshold_scale_ppm = parse(&mut args, &flag)?,
            "--max-mark-index-gap-bps" => max_mark_index_gap_bps = parse(&mut args, &flag)?,
            "--fee-ppm" => fee_ppm = parse(&mut args, &flag)?,
            "--queue-ahead" => queue_ahead = parse(&mut args, &flag)?,
            "--trade-through" => trade_through = parse(&mut args, &flag)?,
            "--market-to-decision-ms" => market_to_decision_ms = parse(&mut args, &flag)?,
            "--decision-to-exchange-ms" => decision_to_exchange_ms = parse(&mut args, &flag)?,
            "--cancel-to-exchange-ms" => cancel_to_exchange_ms = parse(&mut args, &flag)?,
            "--quote-reprice-min-interval-ms" => {
                quote_reprice_min_interval_ms = parse(&mut args, &flag)?
            }
            "--dynamic-capital-refresh-ms" => dynamic_capital_refresh_ms = parse(&mut args, &flag)?,
            "--duration-secs" => duration_secs = parse(&mut args, &flag)?,
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if symbols.is_empty() {
        return Err("--symbols cannot be empty".to_owned());
    }
    Ok(Args {
        experiment_version,
        environment,
        anchors,
        index_anchors,
        symbols,
        output_root,
        capital_usdt,
        entry_threshold_bps,
        threshold_scale_ppm,
        max_mark_index_gap_bps,
        fee_ppm,
        queue_ahead,
        trade_through,
        market_to_decision_ms,
        decision_to_exchange_ms,
        cancel_to_exchange_ms,
        quote_reprice_min_interval_ms,
        dynamic_capital_refresh_ms,
        duration_secs,
    })
}

fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn parse<T: FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String>
where
    T::Err: std::fmt::Debug,
{
    next(args, flag)?
        .parse()
        .map_err(|e| format!("invalid {flag}: {e:?}"))
}
fn parse_decimal(value: &str, scale: u32) -> Result<i64, String> {
    let value = value.trim();
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    let fraction_len = fraction.len() as u32;
    if parts.next().is_some()
        || whole.is_empty()
        || fraction.len() > scale as usize
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("expected a positive decimal".to_owned());
    }
    let unit = 10_i128.pow(scale);
    let whole = whole
        .parse::<i128>()
        .map_err(|_| "decimal overflows".to_owned())?;
    let fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<i128>()
            .map_err(|_| "decimal overflows".to_owned())?
    };
    let scaled = whole
        .checked_mul(unit)
        .and_then(|v| v.checked_add(fraction_value * 10_i128.pow(scale - fraction_len)))
        .ok_or_else(|| "decimal overflows".to_owned())?;
    i64::try_from(scaled).map_err(|_| "decimal overflows".to_owned())
}

struct PaperLabInstanceGuard {
    path: PathBuf,
    pid: u32,
}

impl Drop for PaperLabInstanceGuard {
    fn drop(&mut self) {
        let owned_by_me = fs::read_to_string(&self.path)
            .ok()
            .and_then(|contents| contents.lines().next().map(str::to_owned))
            .and_then(|value| value.parse::<u32>().ok())
            == Some(self.pid);
        if owned_by_me {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn claim_single_paper_lab_instance() -> Result<PaperLabInstanceGuard, String> {
    let path = env::temp_dir().join("anchorbell-paper-lab.lock");
    let pid = process::id();

    for _ in 0..3 {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                writeln!(file, "{pid}").map_err(|error| {
                    let _ = fs::remove_file(&path);
                    format!("cannot write paper-lab instance lock: {error}")
                })?;
                return Ok(PaperLabInstanceGuard { path, pid });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let old_pid = fs::read_to_string(&path)
                    .ok()
                    .and_then(|contents| contents.lines().next().map(str::to_owned))
                    .and_then(|value| value.parse::<u32>().ok());
                if let Some(old_pid) = old_pid.filter(|old_pid| *old_pid != pid) {
                    if paper_lab_process_matches(old_pid) {
                        terminate_paper_lab_process(old_pid);
                        std::thread::sleep(Duration::from_millis(500));
                        if paper_lab_process_matches(old_pid) {
                            return Err(format!(
                                "cannot clean up previous AnchorBell paper-lab process {old_pid}"
                            ));
                        }
                    }
                }
                let _ = fs::remove_file(&path);
            }
            Err(error) => {
                return Err(format!("cannot claim paper-lab instance lock: {error}"));
            }
        }
    }

    Err("paper-lab instance lock is contended".to_owned())
}

#[cfg(windows)]
fn paper_lab_process_matches(pid: u32) -> bool {
    let script = format!(
        "$p=Get-CimInstance Win32_Process -Filter \"ProcessId={pid}\" -ErrorAction SilentlyContinue;          if ($p -and $p.Name -eq 'anchorbell_paper_lab.exe' -and          $p.ExecutablePath -like '*AnchorBell*') {{ exit 0 }} else {{ exit 1 }}"
    );
    process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn paper_lab_process_matches(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/cmdline"))
        .map(|command_line| command_line.contains("anchorbell_paper_lab"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn terminate_paper_lab_process(pid: u32) {
    let _ = process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(not(windows))]
fn terminate_paper_lab_process(pid: u32) {
    let _ = process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
}

fn print_usage() {
    eprintln!("usage: anchorbell_paper_lab [--experiment-version M6] --index-anchors [--environment production] [--symbols S1,S2] [--output-root PATH] [--capital-usdt N] [--quote-reprice-min-interval-ms N] [--dynamic-capital-refresh-ms N] [--duration-secs N]");
    eprintln!(
        "defaults: shared feed + F1..F6 and reverse R6..R1; M0 is retired; M6 uses dynamic capital; queue/latency are explicit realism controls"
    );
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    print_usage();
    process::exit(2);
}
