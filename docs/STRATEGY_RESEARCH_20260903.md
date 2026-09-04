# AnchorBell 策略研究与优化记录（2026-09-03）

## 目标与不可变原则

本轮只优化可变的信号、阈值和启动可靠性，不改变核心方法：股票收盘价是主锚点；只做 maker/post-only；行情、锚点、风控或执行状态未知/过期时 fail-closed；股票开盘和资金费截止前必须降风险/清仓；paper 不读取凭证、不调用下单接口。

## 研究结论

- 库存风险研究表明，报价应随库存和剩余风险时钟动态调整，而不是单一静态价差。
- AAAI-25 的在线学习做市研究支持硬库存边界与软库存惩罚并行；当前先使用可解释的软惩罚，不把黑盒模型放进实盘热路径。
- 近年的 Binance 永续做市研究强调：队列位置、盘口不平衡、延迟和成交后的 markout 会共同决定真实收益；高成交率不等于高净收益。
- 队列/延迟回放工具的共同结论是，独立模拟价格和成交会高估 maker 策略；因此 paper 继续保留保守成交模型，后续用真实记录校准。
- 鲁棒做市研究明确提出“暂不报价”是有效动作；当前硬门禁继续优先于追求交易次数。

## 本轮落地

1. paper、Testnet、live 的共享路径加入滚动绝对收益波动 EWMA。
2. 自适应阈值加入波动、价差、锚点不确定性、费用、统计置信度和方向性盘口逆向选择项。
3. 盘口不平衡对面临薄队列的一侧增加惩罚，避免只因偏离大就接逆向流。
4. 修正流动性惩罚使用“实际压缩后的订单量”，避免目标量大但实际小单仍被错误加上 80–100bp 惩罚。
5. paper 启动时公共元数据请求失败自动重试三次；Dashboard 优先使用当前 review 构建产物，避免误启动旧 binary。

## 暂不采用

暂不把 RL、Hawkes 或在线黑盒策略直接接入 live。它们需要更长时间序列、真实队列/延迟数据、样本外验证和可回滚参数，否则会削弱可解释性与 fail-closed 边界。

## 0903 性能与解耦实验架构

- 新增 `anchorbell_paper_lab`：生产环境只读公共行情由两条共享流采集（bookTicker 与 mark/aggTrade），事件只解析一次，再以借用引用扇出到各个独立 PaperEngine。
- 每个实验拥有独立策略状态、订单生命周期、持仓、PnL、fees、funding、records.jsonl 与 metrics.json；共享行情仅保存一份，避免十个进程重复网络连接与市场落盘。
- M0 已因仓位失控风险退役，不再重跑；从 M1 起前向消融固定为 F1_m1、F2_m2、F3_m3、F4_m4、F5_m5，反向消融固定为 R5_m5、R4_m4、R3_m3、R2_m2、R1_m1。M5 作为本轮 paper-lab/shadow challenger，生产 paper 默认仍为 M4，未经样本外验证不进入 Testnet/Production。
- 写盘使用有界队列、1 MiB 异步缓冲和批量 flush；metrics 采用临时文件写入后原子替换。行情队列、共享写盘和实验账本任一发生丢弃都报告失败，禁止静默丢数据。
- 运行时继续保留硬风控：A/H 股日历、午间可交易阶段、开盘前降风险、资金费截止、mark/index 一致性、锚点新鲜度和 maker-only；性能优化不得放宽这些门禁。
- queue ahead、trade-through、行情到决策延迟、决策到交易所延迟、撤单延迟和 maker fee 均作为显式参数写入配置；未用真实成交事件校准前，不把任何收益结果视为稳定正期望。

## 0903 共享基础设施重构

- 行情订阅分片统一由 `BinanceMarketConfig::for_symbols` 构造；paper、paper-lab 与 live 仅选择 feed 类型和执行适配器，不再分别拼接订阅与分片逻辑。
- Binance 事件到 JSONL 的序列化统一放在 `market::recorder::market_event_to_json`，保证 live 采集、paper 记录和 replay 输入格式一致。
- 异步 JSONL 写入和 metrics 原子写入统一放在 `runtime::io`；不同运行模式只提供输出路径、队列容量和刷新策略。
- 三种模式的职责边界固定为：`market` 负责真实事件，`strategy` 负责可解释决策，`PaperEngine` 负责纸面成交/账本，`replay` 负责历史驱动，`execution` 负责真实账户边界。
- 任何模式都不能通过复制一套策略绕过共享风控；实盘最终只替换成交执行适配器，paper/backtest 使用同一决策与账本语义。

## 0903 自动代理绑定与全量并行运行

- `network::resolve_http_proxy` 现在是所有 Binance 公共行情、元数据、FX 和 WebSocket 客户端的统一入口；显式代理配置优先，未配置时按“本机常见 HTTP 代理端口真实 CONNECT 探测 → 标准代理环境变量 → 直连”顺序选择。
- 自动探测的常见本机端口为 `7890/7891/7892/10809/10808/1080/8888`。探测只发送到 `fapi.binance.com:443` 的 HTTP `CONNECT`，不读取或输出账号凭证；当前机器已验证自动绑定 FlClash `127.0.0.1:7890`。
- 代理结果按进程缓存，避免每条行情重复探测；显式 `--proxy` 仍可用于受控覆盖。若代理在进程运行期间切换，需通过前端重启对应运行实例重新探测。
- 本次全量实验并行运行：一个 `anchorbell_paper` 作为生产行情＋PaperEngine 参考基线，另一个 `anchorbell_paper_lab` 在一条共享双 feed 事件流上并行维护十个独立账本：前向消融 `F1_m1`–`F5_m5`，反向消融 `R5_m5`–`R1_m1`。每个账本独立记录订单、成交、持仓、PnL、费用和 metrics；每次运行自动生成独立目录与 `run-manifest.json`，历史版本和历史运行均不覆盖。
- M4 历史运行输出保留在既有 `target\\paper-lab-*` 目录；M5 当前合并后的运行输出保留在 `target\\paper-lab-20260904-m1m5-v10`，后续运行必须使用新的独立目录，不能覆盖历史结果。实验引擎内部共享行情解析和网络连接，基线 paper 保持独立，便于比较“正式运行链路”和“实验矩阵”。
- 当前已观察到 lab 产生纸面订单和成交；任何短期 PnL 只用于运行验证，不能作为稳定盈利或实盘正期望证明，必须按完整样本和成本/延迟模型评估。
- 验证状态：GNU toolchain 下 workspace 单元测试 `237 passed; 0 failed`；GNU release 版 `anchorbell_paper`、`anchorbell_paper_lab` 已构建成功。rustfmt 组件缺失只影响 `fmt --check`，不影响测试和构建。

## 0903 深度研究：收益、回撤与黑天鹅生存

### 研究依据与可迁移结论

- [Cont–Kukanov–Stoikov 的订单簿事件价格冲击研究](https://arxiv.org/abs/1011.6402)显示，短周期价格变化与订单流不平衡近似线性相关，冲击系数随市场深度下降而增大。对 AnchorBell 的直接含义是：盘口不平衡不能只作为入场加分项，还必须同时进入“可承受库存”和“冲击损失预算”。
- [Market Simulation under Adverse Selection](https://arxiv.org/html/2409.12721v2)指出，把价格过程与成交过程独立模拟会显著高估短线策略；成交概率、成交后的不利变动必须绑定在同一事件流中。当前 paper 的 queue/trade-through/latency 参数必须从零值基线升级为校准区间，并记录成交后 markout。
- [Fill Probability vs. Post-Fill Returns](https://arxiv.org/html/2502.18625v2)说明成交率、成交价格和成交后收益相互冲突；不能以 fills 多作为优化目标。实验主指标应改为成本后 markout、单位风险收益和尾部损失。
- [Event-Time Anchor Selection](https://arxiv.org/html/2507.05749v2)支持按事件时间而非固定时钟评估锚点有效性。我们的固定官方收盘锚点原则不变，但必须区分“锚点身份固定”和“锚点在当前事件状态下是否仍可交易”。
- [Optimal Execution with Passive Market Impact](https://arxiv.org/html/2607.28323v1)将被动成交概率随报价距离衰减、订单流不平衡引起的短期价格响应纳入同一模型。可用于校准报价层，但不把任何未经真实成交校准的公式直接当作盈利保证。
- [Dealing with the Inventory Risk](https://arxiv.org/abs/1105.3115)及库存约束做市框架共同支持“硬边界 + 软库存惩罚”：硬边界防止失控，软惩罚让策略在边界前逐渐停止增加风险。
- Binance 官方文档显示，USDⓈ-M 的交易规则应以 `exchangeInfo` 为准；GTX 是可用的被动订单时效选项，`reduceOnly` 是明确的减仓语义。Binance 还明确要求收到 429 后退避，持续违规会导致 418 IP 封禁；symbol 级 ADL 风险评级会综合保险基金、仓位集中度、深度、波动率、杠杆、未实现盈亏和保证金使用率，并每 30 分钟更新。[Exchange Information](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data)、[Trade API](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/trade)、[General Info](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/general-info)、[ADL Risk](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data)。

### 当前源码审计发现

- `M0Fixed` 路径调用旧的固定阈值接口，不把 `max_position` 传入决策；它适合作为危险基线，不适合作为候选实盘策略。
- 自适应路径现在显式加入 deadline 与 inventory 惩罚；M5 在此基础上增加可审计的 tail-risk surcharge，并把高压力状态映射为缩量报价、只减仓或停止新报价。
- 当前 PaperEngine 的数量上限是每标的硬上限，不等于组合保证金/名义价值/清算距离上限；实验输出已经出现单个标的仓位远超其目标数量的现象，必须先修复为成交前和成交后的双重硬限额。
- 当前 paper-lab 默认 `queue_ahead=0`、三类延迟均为 0，仍属于乐观管线基线；M5 的尾部保护不能替代真实 queue/latency 校准，收益结果只能用于版本间相对比较，不能直接估计实盘收益。
- 当前指标有 PnL、费用、资金费、仓位和历史点，但还缺少成交后 1s/5s/30s markout、按时段/标的/风险状态归因、峰值回撤持续时间、尾部损失分位数、订单拒绝与撤单原因分解。这些缺失会让“收益变高”与“风险被隐藏”难以区分。

### 数学契约：保留核心原则后的可执行优化

固定锚点 \\(A_t\\) 不变，入场方向仍只允许在合约价格相对锚点出现足够偏离时做 maker。新的可解释门槛定义为：

\\[
E^{side}_t \\ge C_t + U_t + S_t + L_t + AS^{side}_t
       + I^{side}_t + D_t + T_t
\\]

其中：

- \\(E^{side}_t\\)：以可成交的最优 maker 价格计算的锚点边际；
- \\(C_t\\)：maker 费、预计资金费、手续费变化和可量化交易成本；
- \\(U_t\\)：mark/index、FX、收盘锚点年龄及来源质量带来的不确定性；
- \\(S_t\\)：价差和最小跳动造成的边际误差；
- \\(L_t\\)：订单量相对可见深度以及预估冲击的惩罚；
- \\(AS^{side}_t\\)：订单流不平衡、薄侧队列和成交后 markout 校准的逆向选择惩罚；
- \\(I^{side}_t\\)：仓位风险惩罚，增仓方向随 \\( |q|/q_{max} \\) 非线性上升，减仓方向不加该项；
- \\(D_t\\)：距股票开盘/资金费结算的剩余时间与平仓可执行性的风险；
- \\(T_t\\)：尾部状态惩罚；常态为 0，冲击状态下迅速增加，不能靠降低阈值抵消。

数量必须同时满足：

\\[
q_{order} \\le \\min(q_{symbol},q_{portfolio},q_{margin},q_{depth},q_{tail})
\\]

其中 \\(q_{symbol}\\) 是标的硬仓位界限，\\(q_{portfolio}\\) 是组合净/毛名义和相关集中度界限，\\(q_{margin}\\) 是按 mark price、维护保证金和清算缓冲计算的可用风险，\\(q_{depth}\\) 是可见深度与队列模型限制，\\(q_{tail}\\) 是冲击状态下的缩放上限。任一项未知时取 0，不用乐观估计。

### 黑天鹅保护状态机

`TRADING → CAUTION → REDUCE_ONLY → HALT`，状态只允许由更高层风险管理器提升，恢复必须满足滞回阈值、最小冷却时间和数据连续性。

- `TRADING`：正常报价，但仍受全部成本、库存和组合限额约束。
- `CAUTION`：新单数量缩放，报价更保守；触发条件包括 robust 波动跳变、价差/深度恶化、mark/index 迅速分离、订单确认/撤单延迟异常。
- `REDUCE_ONLY`：撤掉增仓单，只允许 post-only 减仓；到期或风险窗口临近时强制进入。
- `HALT`：撤单并停止新订单；行情、锚点、账户、交易所规则、代理/API 状态任一未知时 fail-closed。

冲击检测优先采用无参数黑盒依赖的 robust 统计：短窗绝对收益相对长期 EWMA/MAD 的倍数、mark/index gap、盘口深度分位数、价差分位数和连续异常计数；使用双阈值滞回，防止在临界点来回切换。检测器不能预测黑天鹅，只能在损失扩大前减少暴露。

### 代码实施优先级

1. 先把 M0 明确标记为危险对照组；所有候选实盘路径统一走自适应决策，不允许固定路径绕过组合限额。
2. 将库存惩罚、截止风险、尾部状态和组合风险预算放进共享 `RiskState`/纯函数；paper、replay、backtest、live 共享同一决策契约。
3. 在订单生成前做预检查，在成交应用后再次做硬校验；为每次阻断记录结构化原因，防止限额因单位换算或部分成交被穿透。
4. 扩展 paper realism：按标的和时段使用 queue ahead、trade-through、决策/交易所/撤单延迟分布；同一公共事件流上绑定成交和 markout，不能独立抽样。
5. 增加成本后 markout、组合收益曲线、最大回撤、回撤持续时间、收益波动、Sharpe/Sortino、Calmar、VaR/CVaR、最差日/时段、尾部情景损失及按状态归因。
6. 对实验使用共同随机数和相同事件流，报告前向/反向消融的配对差值与置信区间；短样本只作为管线健康检查。
7. 实盘前接入 `exchangeInfo` 规则快照、订单计数/429退避、ADL 风险、账户保证金、仓位和用户数据流；任何状态不同步都只能 `NO_ACTION/REDUCE_ONLY/HALT`。

### 验证门槛

- 不接受只看总收益或 fill count；必须同时通过净收益、最大回撤、尾部损失、仓位上限、成交后 markout、成本覆盖率和数据完整性检查。
- 先在真实生产公共行情上跑 paper；再用历史事件 replay 覆盖正常、开盘/收盘、午间、资金费前、断流、代理切换、深度坍塌、mark/index 分离和跳跃冲击情景。
- 任何优化只有在保守成交参数和压力情景下仍改善风险调整收益，才允许进入 shadow；不能用放宽队列、延迟或清仓规则来制造更高收益。
- C++ 暂不是主要矛盾：当前瓶颈是成交现实度、风险边界和数据归因，而不是 Rust 计算吞吐；保持 Rust 共享核心，只有在 profiling 证明某个微秒级热段构成实际瓶颈时再局部替换。

### 2026-09-04 非 M0 并行实验与指标契约

当前纸盘实验明确排除 M0，不再把固定阈值作为候选策略运行。并行账本为：`F1_m1`、`F2_m2`、`F3_m3`、`F4_m4`、`F5_m5`，以及对应的反向消融账本 `R1_m1`–`R5_m5`。所有账本共享同一生产公共行情、FX 流和事件顺序；每个账本独立维护订单、成交、仓位、费用和 PnL。

每个账本的 `metrics.json` 必须同时提供三层数据：

1. 方法/账本总指标：`summary` 保留事件、订单、成交、已实现/未实现、市场/策略/资金费、费用、净 PnL、当前/峰值仓位；`risk_metrics` 新增样本数、观察秒数、总收益率、最大回撤、胜率、平均收益（bps）、Profit Factor、Sharpe 和 Sortino。
2. 股票指标：`symbols[]` 为每个标的的实时状态、锚点/日历/数据质量、买卖边际、完整自适应阈值分解、成交与 PnL；每个元素的 `risk_metrics` 使用该标的独立资金分配计算。
3. 组合指标：`risk_metrics` 使用所有标的净 PnL 曲线计算，作为最终组合比较口径，而不是把各股票收益率简单平均。

Sharpe/Sortino 采用按观察间隔归一的年化计算，至少需要 30 个独立收益采样点；不足时状态为 `insufficient_history` 且比率为 `null`。这避免把几秒钟的偶然成交误报成稳定策略。F5 的尾部附加项来自 robust 波动、mark/index 分离和价差压力，并与仓位缩放/只减仓保护共同生效；不能通过降低基础阈值绕过。

### 2026-09-04 报价生命周期优化

运行样本显示，早期版本的撤单主要来自同方向报价的重复 quote replacement，而非风险反转。这类换价会增加撤单、延迟与队列损耗，却不必然增加有效成交。当前 paper 与共享策略路径加入同方向报价最短驻留时间，默认 `750ms`：原报价剩余量仍足够时保持原价，减少无效撤单/重挂；反向报价、减仓、资金费/股票开盘临界或风险状态变化可立即绕过。

该优化只约束订单生命周期，不放宽固定锚点、maker-only、午间可交易时段、行情新鲜度、mark/index 一致性或任何硬风控。比较时必须同时观察撤单原因、成交率、成交后 markout、费用、净收益、最大回撤与风险调整收益。每个账本继续输出组合层与逐股票 `risk_metrics`；历史不足 30 个 30 秒采样点时保持 `insufficient_history`，Sharpe/Sortino 不填假值。
