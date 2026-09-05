# AnchorBell 实验记录

## 2026-09-04：M6 第一版无限时长模拟盘

- 实验版本：M6
- 实验模式：完整消融矩阵并行，不允许单独运行某一个方法
- 并行 ledger：F1_m1、F2_m2、F3_m3、F4_m4、F5_m5、F6_m6、R6_m6、R5_m5、R4_m4、R3_m3、R2_m2、R1_m1
- 正式标的：CXMTUSDT、UNITREEUSDT、GIGADEVUSDT、HK0625USDT、MINIMAXUSDT、ZHIPUUSDT、ZHONGJIUSDT
- 总资金：约 10,000 元人民币，启动参数为 1,400 USDT
- 资金分配：F1–F5/R1–R5 固定等权控制；F6/R6 使用 M6 动态资金
- 动态资金刷新：默认 60,000 ms；按波动、价差、Mark/Index 偏离和尾部压力计算风险权重
- 模拟器：模拟盘，不提交真实订单；市场到决策 50 ms，决策到交易所 100 ms，撤单到交易所 100 ms，队列前置 1 个单位
- 时长：无限运行（duration_secs=0）
- 构建：GNU Rust
- 进程：anchorbell_simulation_batch.exe，启动时 PID 23864
- 输出目录：target\\simulation-batch-20260904-M6-10000cny
- 指标要求：每个 ledger、每个标的分别持续记录 metrics.json，并使用同一市场事件流比较

后续原则：每次实验必须同时运行 M1 到当前最高版本的全部方法，并保留固定控制组与完整消融结果。

## Simulator optimization S1 (2026-09-03)

- Strategy signals and M1-M6 rules unchanged; simulator work is isolated from strategy optimization.
- Binance diff-depth parser now preserves E/T/U/u/pu and all price/quantity levels.
- SimulationBatch starts depth streams before REST snapshots, buffers events, then seeds sequence-validated local books.
- Depth gaps, crossed books, invalid levels, and missing snapshots fail closed; no continued matching on a broken book.
- Simulation/replay order timing now separates exchange event time from local receipt time; exchange arrival and cancel acknowledgement honor configured latency.
- Seeded local depth limits fills at the order's actual price; top-of-book replay without snapshots keeps explicit top-of-book fallback.
- GNU validation: anchorbell-engine all targets, 242 tests passed.
- The existing PID 23864 M6 full matrix run was not stopped or retrofitted; the next retained run will use S1.
