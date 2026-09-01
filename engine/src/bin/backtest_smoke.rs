use static_anchor_engine::backtest::{
    ConservativeTopOfBook, FillDecision, FillModel, MakerQuote, TopOfBook,
};
use static_anchor_engine::backtest_report::BacktestReport;
use static_anchor_engine::execution::Side;

fn main() {
    let fixture = [
        (99_i64, 100_i64, 10_i64, 8_i64),
        (100, 101, 4, 6),
        (101, 102, 3, 2),
    ];
    let mut report = BacktestReport::default();
    for (bid, ask, bid_qty, ask_qty) in fixture {
        report.record_event();
        let decision = ConservativeTopOfBook.evaluate(
            MakerQuote {
                side: Side::Sell,
                price_ticks: bid,
                quantity: 5,
            },
            TopOfBook {
                bid_price_ticks: bid,
                ask_price_ticks: ask,
                bid_quantity: bid_qty,
                ask_quantity: ask_qty,
            },
        );
        if let FillDecision::Fill { quantity } = decision {
            report.record_fill(quantity, 1, 0);
            report.record_position(quantity);
        }
    }
    println!(
        "events={} fills={} quantity={} fees={} net_pnl={} peak_position={}",
        report.event_count,
        report.fill_count,
        report.filled_quantity,
        report.fees_ticks,
        report.net_pnl_ticks(),
        report.peak_absolute_position
    );
}
