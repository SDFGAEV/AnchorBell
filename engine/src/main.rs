mod core;
mod event;
mod market;
mod orderbook;
mod strategy;
mod execution;
mod runtime;
mod replay;
mod backtest;
mod backtest_report;
mod observability;

fn main() {
    let _orders = execution::OrderManager::new();
    let _runtime = runtime::TradingRuntime::new();
}
