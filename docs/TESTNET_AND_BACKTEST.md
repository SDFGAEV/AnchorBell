# Binance Testnet and Historical Replay

## Testnet boundary

AnchorBell now has an explicit Binance environment model. Testnet and production
carry different REST, market-data WebSocket, and order WebSocket endpoints.

The current gateway is still a deterministic execution boundary. It does not
send keys or orders over the network yet. The next adapter will use Binance's
signed WebSocket API for order operations and the public market streams for
bookTicker and mark price.

Use testnet credentials only. Store API keys in environment variables or a
secret manager; never commit them, print them, or put them in a config file.
## What testnet can validate

Testnet is useful for checking authentication, symbol filters, order rejects,
post-only behavior, cancel/replace handling, reconnects, and the order lifecycle
against exchange acknowledgements.

It cannot prove profitability or reproduce historical fills. Testnet liquidity,
latency, matching, and available symbols can differ from production. Keep the
environment explicit in every run and require a deliberate production switch.

## Replay and backtesting

The engine now accepts a deterministic, timestamp-ordered event stream containing
bookTicker, mark price, and anchor events. A recorded raw Binance WebSocket line
can be decoded into this stream through the replay boundary. EventReplay has no
network dependency: the same input must produce the same strategy decisions and
risk transitions.

The recording format is append-only JSONL: one raw WebSocket message per line,
with the local receipt timestamp retained by the recorder in the next adapter
layer. Files should be immutable once a replay run starts.
For this maker strategy, kline-only backtests are not sufficient. A useful first
dataset is recorded bookTicker plus mark price and anchor snapshots. A more
realistic fill model will additionally need trade/depth observations, local
receipt timestamps, order latency, queue position assumptions, and cancel timing.

The planned pipeline is:

1. Record public Binance market streams to immutable event files.
2. Normalize raw or combined WebSocket JSON into typed replay events.
3. Replay events through the same strategy, risk, and lifecycle code.
4. Report fills, inventory, exposure time, fees, rejects, and drawdown.
5. Compare replay decisions with testnet acknowledgements before any production
permission is considered.

Replay must fail fast on out-of-order timestamps so an accidental clock or file
merge bug cannot silently change results.
## Safety gates

- Default environment: testnet.
- Production requires an explicit configuration change.
- Post-only is mandatory for entry orders.
- The session risk gate must flatten before the underlying market reopens.
- Stale market data, invalid anchors, position caps, or lifecycle rejects halt
  new entries.
- Backtest results must record assumptions; they are not live performance claims.
