# AnchorBell Roadmap

本文件是 AnchorBell 的总规划与执行顺序。它记录目标、不可变原则、外部项目调研结论、系统边界、阶段任务和验收证据。

AnchorBell 面向 Binance equity perpetual 的休市期间短周期 maker 均值回归研究与执行。收益窗口可以从几秒到几小时，但系统必须优先保证时效性、风险边界、可复现性和证据完整性。

## Current closure status

截至 2026-09-01，仓库内的代码闭环已覆盖 P0/P1/P2/P3/P5/P6 的核心契约：行情解析与重连、maker-only 执行边界、生命周期与恢复、资金费率感知 replay/backtest、bounded runtime dispatch、审计序列、严格 Clippy 门禁和吞吐 smoke 均已有实现与测试。

P4 的“真实 Binance Testnet 证据”不能由本地单元测试伪造。2026-09-01 已在远端 Windows 通过显式 HTTP CONNECT 代理取得 15 个 BTCUSDT 公共 `bookTicker`/`markPrice` 事件，公共行情链路闭合；未配置代理时仍按连接门禁 fail-closed。认证、下单、部分成交、撤单、重连和账户 reconciliation 必须使用专用 Testnet 凭证并保留脱敏原始证据。Production 仍保持显式关闭。


## Decision authority

所有外部项目、行业惯例、性能技巧和开源实现都只能作为参考。最终决策唯一服从本项目已确认的要求：

- Rust-first；Rust 负责实盘核心，Python 只负责研究分析。
- 低延迟、事件驱动、热路径内存化，持久化必须异步旁路。
- 只做 maker；不得因为外部实现方便而引入 taker 或隐式市价成交。
- 策略、风险、执行、行情、回测和运行时高度解耦。
- Testnet 优先；Production 默认关闭并且必须显式授权。
- 不确定状态 fail-closed，不能自动增加风险。
- 不为旧接口保留不必要的兼容层；优先清晰、typed、可演进边界。
## Non-negotiable invariants

1. 任何开仓都必须经过 maker-only policy、risk gate 和有效 anchor 校验。
2. session 结束前必须撤单并平仓。
3. stale market data、invalid anchor、未知订单状态和恢复失败都不能增加风险。
4. 交易热路径不得同步写 SQLite、JSONL 或其他磁盘介质。
5. 凭证只能来自受控运行时输入，不能进入代码、日志、issue、commit 或回放数据。
6. Binance network I/O 只能存在于 adapter；strategy 不得直接开 socket 或访问 exchange state。
7. live、paper、replay、backtest 使用同一组核心事件和策略契约。
8. 所有回测必须声明数据、延迟、排队、成交、手续费、资金费率和时间假设。
9. Testnet 和 Production 必须使用不同的 typed environment。
10. 任何外部项目的功能吸收都不得违反以上原则。

## Configurable dimensions

以下内容可以替换或配置，但不能绕过不变量：

- anchor 来源与有效时间
- session 窗口
- deviation 阈值
- quote 宽度与 inventory skew
- symbol allowlist
- 最大仓位与订单数
- fill、fee、funding 和 latency model
- reconnect backoff
- report format
- paper、replay、backtest、testnet runtime mode
## GitHub reference review

### NautilusTrader

Repository: https://github.com/nautechsystems/nautilus_trader

可吸收：

- deterministic event-driven core
- live execution 与 historical simulation 的同构思路
- data、execution、risk、portfolio、adapter 的明确边界
- Rust 核心与研究控制面的分离
- adapter contract 和 integration test 的组织方式

不能直接照搬：

- 多资产、多 venue 和通用平台复杂度不是 AnchorBell 当前目标。
- Python 不能进入 AnchorBell 的实盘热路径。
- 任何会削弱 maker-only 或 session flatten 的通用能力都不引入。

### hftbacktest

Repository: https://github.com/nkaz001/hftbacktest

可吸收：

- Binance Futures 的真实 tick/order-book 回测方向
- queue position、latency、limit order 和 maker fill 的显式建模
- Rust 高性能回放路径
- 用 full tick data 而不是只用 K 线评价 maker 策略

不能直接照搬：

- 它的策略和数据模型只能作为 fill/replay 参考，不能替代 AnchorBell 的 anchor、risk 和 lifecycle 契约。
### Hummingbot

Repository: https://github.com/hummingbot/hummingbot

可吸收：

- exchange connector 的统一 data/execution boundary
- REST、WebSocket、订单和账户状态的适配器分层
- connector capability 的显式表达
- connector-level integration testing

不能直接照搬：

- Python 主体不符合 Rust-first 热路径要求。
- 多交易所扩展不是当前优先级。
- 它的策略行为不能覆盖 AnchorBell 的 equity close anchor 和 session flatten。

### Barter

Repository: https://github.com/barter-rs/barter-rs

可吸收：

- Rust crate/module 解耦
- live、paper、backtest 的共享抽象
- 可替换 data feed、execution、strategy 组件
- 小型 typed library 的组合方式

不能直接照搬：

- 先保持 AnchorBell 的单一策略领域和强安全 gate，不为了“通用框架”提前扩大抽象。
- 任何隐式状态共享、同步持久化或弱类型配置都不采用。

### 参考结论

外部项目共同证明了三件事：事件驱动、live/backtest parity、adapter isolation 值得吸收；但 AnchorBell 的 maker-only、anchor validity、session flatten、热路径隔离和 Rust-first 优先级高于它们的通用设计。
## Target architecture

Market I/O -> typed market event -> bounded dispatcher -> strategy decision -> risk gate -> order intent -> execution adapter -> lifecycle event -> order manager

异步旁路从 typed event 和 lifecycle event 获取：

- JSONL/parquet recorder
- metrics and tracing
- audit log
- replay checkpoints
- session report

核心层不得依赖 recorder、数据库、网络客户端或 Python。

### Planned module ownership

| Area | Owns |
| --- | --- |
| market | decode, timestamps, subscriptions, reconnect input |
| strategy | anchor, session, deviation, quote, inventory policy |
| execution | signing, order transport, lifecycle, reconciliation |
| risk | stale, limits, flatten, production safety |
| replay | ordered event ingestion and deterministic clock |
| backtest | fill, queue, latency, fee, funding assumptions |
| runtime | composition, bounded queues, task supervision |
| observability | metrics, tracing, audit and health |
| research | Python-only analysis and report consumption |

## Delivery phases

### P0: real market adapter

Implement WebSocket connection state machine, bookTicker/markPrice decoding, ping/pong, timeout, reconnect, multi-symbol subscriptions, bounded queues, sequence/timestamp validation and async recording.

Acceptance: malformed input is rejected; stale input is detected; reconnect does not duplicate events; strategy is never blocked by recorder failure.
### P1: signed Testnet execution

Implement Binance WebSocket order adapter, HMAC signing, request/response correlation, order.place, order.cancel, order.status, open-orders and account reconciliation, typed exchange errors, filter validation and idempotent cancellation.

Acceptance: all requests are typed and signed; unknown responses cannot create fills; duplicate responses are harmless; production endpoints cannot be selected by Testnet configuration.

### P2: lifecycle and recovery

Implement order/position reconciliation, unknown-order handling, crash-safe session state, recovery state machine, graceful shutdown, cancel-before-open-risk policy and forced flatten.

Acceptance: after disconnect or restart the system first stops new risk, then reconciles exchange truth, then cancels or flattens according to policy.

### P3: complete backtest and replay

Add full tick/order-book fixtures, event-time and receipt-time replay, queue position, maker latency, partial fills, cancel latency, fees, funding, stale data, gaps and session boundaries.

Every report includes dataset digest, configuration digest, strategy version, commit SHA, model assumptions and time range.

### P4: Testnet evidence

Run authentication, post-only reject, accepted maker order, partial fill, full fill, cancel, reconnect, timeout, reconciliation, stale-data halt, limit rejection and session flatten scenarios.

No production enablement is implied by passing Testnet. Testnet evidence is a prerequisite for any later review.

### P5: performance and reliability

Benchmark parser throughput, event-to-decision latency, decision-to-submit latency, queue saturation, reconnect convergence, allocation rate and recorder isolation.

Optimize lock contention, allocation, serialization, queue behavior and connection reuse in that order. Preserve behavior while optimizing.
### P6: observability and release

Add structured tracing, decision audit, lifecycle audit, health/readiness/liveness, metrics histograms, redaction, graceful shutdown, dependency audit, SBOM, reproducible build, CHANGELOG, operator runbook and release checklist.

Release stages:

1. research-only
2. paper gateway
3. replay/backtest verified
4. Binance Testnet verified
5. production-disabled release
6. separate production safety review, only if ever requested

## Test system

Required layers:

- pure unit and property tests
- parser fuzz tests
- contract tests for market and execution adapters
- deterministic replay tests
- fake-exchange lifecycle tests
- risk invariant tests
- bounded-queue and concurrency tests
- reconnect and crash-recovery tests
- Testnet integration tests
- golden backtest fixtures
- performance regression tests

Hard gates:

- no taker order can be produced
- no invalid anchor can open risk
- no session can end with unmanaged exposure
- no duplicate request can duplicate risk
- out-of-order replay is rejected
- persistence failure cannot alter trading decisions
- production cannot activate without explicit policy

## Definition of Done

A phase is complete only when code, focused tests, documentation, exact commit SHA and reproducible verification command are present. “代码能编译”不等于完成；真实 adapter、恢复证据、延迟数据和风险行为必须分别验证。

当前 Cargo 全量测试曾受远端 crates.io 依赖不可用影响；CI 已配置，后续必须在依赖可取得的环境完成 fmt、test、clippy 和集成验证。

## Review rule

每次吸收外部项目之前，先回答：

1. 它解决的是 AnchorBell 的哪一个已定义问题？
2. 是否保持 Rust-first 和热路径隔离？
3. 是否保持 maker-only、fail-closed、session flatten？
4. 是否增加了不必要的通用复杂度？
5. 能否用 typed contract、测试和可复现证据证明没有破坏行为？

若不能证明，默认不吸收。
