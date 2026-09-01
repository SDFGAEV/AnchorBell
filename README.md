# AnchorBell

AnchorBell is an open-source, maker-only research and execution engine for
short-horizon trading of Binance equity perpetual contracts around
equity-market closing-price anchors.

## Core idea

During the underlying equity market's closed session, the perpetual contract
may temporarily deviate from the last reliable equity-market close. AnchorBell
researches passive post-only quotes around that anchor and exits before the
underlying market reopens.

## Principles

- Binance equity perpetual contracts only.
- Maker-only execution; every live order must be post-only.
- Short-horizon exposure during the underlying market's closed session.
- Positions must be closed before the underlying market reopens.
- Rust-first low-latency execution boundaries.
- Research, market data, strategy, execution, risk, and recovery remain decoupled.

## Status

Architecture scaffold with typed Binance market parsing, static anchor/session
risk controls, deterministic maker order lifecycle, explicit testnet endpoints,
and timestamp-ordered event replay. Network signing, live order submission,
recording, and fill-model validation remain to be implemented.

## License

MIT
