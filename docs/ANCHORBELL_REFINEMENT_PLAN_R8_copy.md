# AnchorBell 方案持续打磨文档

> 用途：记录 AnchorBell 模拟器与策略的多轮方案审查、决策、实现边界和验证结果。
>
> 规则：后续每一轮继续修改本文件；模拟器与策略必须分开记录；每轮方案先审查、再实现、再跑完整 M1–Mxx 矩阵。

## 0. 文档状态

- 当前轮次：Round 8
- 日期：2026-09-04
- 当前实验基线：M6 第一版完整矩阵模拟盘（已退出）
- 最近输出目录：target\\paper-lab-20260904-M6-10000cny
- 最近进程：anchorbell_paper_lab.exe，PID 23864；2026-09-04 复查时已不存在
- 末条记录为“paper lab stopped”，各 ledger 同步收尾；当前记录未保存触发来源/退出码，不能判断是人工信号、父进程结束还是内部退出
- 最近进程使用：S1 深度模拟器改动之前的旧二进制
- Round 2 状态：数学方案与实施契约完成
- Round 3 状态：日志、归因、指标与统计比较契约完成
- Round 4 状态：S2/M7 数学求解层完成第四轮打磨
- Round 5 状态：模拟器数字孪生、策略控制、因果日志与真实性指标完成第五轮联合打磨
- Round 6 状态：固定锚点均值回归核心假设的独立统计验证协议完成
- Round 7 状态：结构可识别性、有限样本模拟器不确定性、稀有破产事件与时间一致鲁棒控制完成第七轮联合打磨
- Round 8 状态：交易所契约更新、反事实执行世界、部分识别 OPE、安全策略升级与价值信息实验设计完成第八轮联合打磨；尚未修改策略/模拟器代码，尚未启动新实验

## 1. 不可变原则

### 1.1 交易对象与核心机制

- 交易对象为 Binance USDⓈ-M TradFi 永续合约。
- A 股/港股官方收盘价是固定锚点。
- 股票闭盘期间，合约相对固定收盘锚点的偏离构成均值回归信号来源。
- 固定锚点不能被合约价格反向改写，不能使用未来信息或事后修正。
- M1 到当前最高版本必须在同一公共事件流上并行运行。
- 每一版实验、每一个 ledger、每一个标的的原始记录和指标必须保留。

### 1.2 安全与执行边界

- 行情、锚点、FX、资金费、账户、交易所规则、订单状态任一未知或过期时，默认不增加风险。
- 正常报价默认只允许 Maker/Post-only。
- 股票开盘前、资金费截止前和明确的极端风险状态必须降低风险。
- Paper、Replay、Backtest、Live 使用同一策略决策契约；只有执行适配器不同。
- Paper 不读取真实凭证，不调用真实下单接口。
- 不把短期收益、成交率或单次实验结果当作稳定正期望证明。
- 不使用深度学习替代当前可解释策略热路径；新方法必须可审计、可回滚、可消融。

## 2. 可变部分

### 2.1 模拟器可变部分

- 事件接收、排队、乱序、重复、丢失、断线和重连语义。
- 深度盘口、队列位置、同价位撤单/新增/成交和流动性坍塌。
- 订单 ACK、Post-only 拒绝、撤单 ACK、撤单竞态、部分成交和对账。
- 市场到决策、决策到交易所、撤单到交易所的延迟分布。
- 手续费、资金费、合约规格、保证金和清算缓冲。
- 共享事件流、写盘、回放、故障注入和指标口径。

### 2.2 策略可变部分

- 入场门槛、信号置信度和方向性逆向选择惩罚。
- 仓位目标、组合集中度、动态资金和风险预算。
- 固定锚点下的残差状态、单边扩散识别和持仓时限。
- 正常减仓、风险减仓、资金费前减仓和极端退出策略。
- M1–M6 的可解释消融组件；核心锚定原则不得被改变。

## 3. Round 1：当前结果的事实基线

本轮采用的 M6 最新快照约为：策略损益 +0.48 USDT，市场持仓损益 -5.17 USDT，资金费 -0.21 USDT，手续费 -0.23 USDT，净损益 -5.13 USDT。M6 的主要亏损来自 HK0625USDT 和 MINIMAXUSDT：

- HK0625：固定锚点约 42.77，Mark 约 40.28，系统持有多头，单标的净损益约 -4.35 USDT。
- MINIMAX：固定锚点约 45.87，Mark 约 48.21，系统持有空头，单标的净损益约 -4.59 USDT。
- 两者的 Mark/Index 仍然接近，行情质量被判定为 Fresh，但锚点残差已经约为 5%–6%。
- 这说明当前数据质量门禁正常工作，但没有识别“固定锚点可能暂时失效或持续偏离”的风险。

M6 动态资金快照还出现了目标资金与实际名义仓位不一致：

- CXMT：分配约 125 USDT，实际持仓约 200 USDT。
- MINIMAX：分配约 117 USDT，实际持仓约 283 USDT。
- UNITREE：分配约 235 USDT，实际持仓约 296 USDT。
- 动态资金当前主要限制后续加仓，没有对已有持仓形成强制硬上限。

M6 约有 3322 次 capital_rebalance、550 次挂单和 528 次撤单。60 秒刷新下几乎每次刷新都改变分配，说明动态资金缺少滞回、最小变化阈值和冷却机制。## 4. Round 1：已确认问题

### 4.1 策略与执行耦合问题

当前减仓分支存在确定性方向矛盾：

- 多头减仓意图使用 bid 价格卖出。
- 空头减仓意图使用 ask 价格买入。
- Post-only 校验却要求卖价不低于 ask、买价不高于 bid。
- 因此严格 Maker 校验会拒绝上述减仓单。

结果是 M5/M6 可以进入 reduce-only，但并不一定能真正减少已有仓位。必须在下一次实验前修复并单测。

### 4.2 策略风险模型问题

- 当前尾部风险主要监控短期波动、Mark/Index 偏离和价差，没有把 Anchor residual、残差速度和持续时间作为核心风险。
- 以“偏离越大、收益空间越大”的线性直觉处理固定锚点，无法识别单边新闻/重新定价行情。
- 动态资金风险分数没有充分包含锚点残差、资金费绝对成本、当前持仓、保证金和组合集中度。
- 动态目标下降时没有明确的“先减仓至目标、再恢复增仓”状态机。
- rejected_entries 主要记录内部策略门禁触发，不应继续与交易所拒单混用。

### 4.3 模拟器时间与撮合问题

- 当前事件循环中调用 now_ms() 的位置更接近“主循环取出事件时间”，不一定是真实 WebSocket 收包时间。
- 订单到达时间使用本地接收时钟，而成交判断仍使用 trade.event_time_ms，存在时钟语义混用。
- BookTicker/Mark 触发 rebalance 时部分使用交易所事件时间，撤单和动态资金又使用本地时间。
- 固定 queue_ahead 不能表达同价位新增、撤单、成交和自身排队位置变化。
- 深度断档目前偏向终止整个实验，尚未实现标的级冻结、重新快照、对账和恢复。
- 共享事件丢失不能等到实验结束才发现；无限运行中应立即冻结或停机。
- 订单生命周期还需要更完整地区分意图、发送、交易所确认、拒单、部分成交、撤单请求和撤单确认。## 5. 模拟器方案：下一轮实施边界

### 5.1 统一事件时钟与事件封装

所有实时和回放事件统一封装为 MarketEnvelope：

- 原始事件内容；
- exchange_event_time；
- exchange_transaction_time；
- local_receive_time；
- ingress_sequence；
- connection/shard/feed 标识；
- 是否经过重连、缓存或重放。

订单到达、成交资格、撤单竞态和策略决策全部基于明确的同一模拟时间轴。禁止直接拿交易所事件时间与本地到达时间混合比较。

### 5.2 盘口与队列

- REST snapshot 与 diff-depth 序列严格校验。
- 盘口断档后只冻结受影响标的，撤销其增加风险的订单并重新同步。
- 在每个价格档维护显示数量、估计前方队列和自身剩余队列。
- 处理同价位新增、减少、撤单和 aggressed trade 的先后关系。
- 记录成交前盘口、成交后盘口和成交后 1s/5s/30s markout。
- 真实深度未知时采用保守区间，不用乐观的 top-of-book 直接成交。

### 5.3 订单生命周期与故障

- 建立统一 OrderState：Intent、PendingSubmit、New、PartiallyFilled、Filled、PendingCancel、Canceled、Rejected、Unknown。
- Post-only 在订单到达交易所时重新检查，不能只在本地生成时检查。
- 撤单请求到确认之间允许真实竞态成交。
- 交易所用户数据流与 REST open-orders/position snapshot 对账。
- 订单状态不确定时进入 NO_ACTION 或 REDUCE_ONLY，不能继续增加仓位。
- 共享事件丢失、深度断档、写盘丢失和对账失败必须即时记录 simulator_halt。

### 5.4 断线与恢复

- 断线不应默认伪造连续行情。
- 受影响标的取消增仓意图并冻结撮合。
- 重新连接、REST 快照、序列接续、订单/持仓对账成功后才恢复。
- 恢复过程和恢复前后的数据空洞必须进入指标和报告。
- 其他标的在风险隔离允许时继续运行；全局账户异常时才升级为全局 HALT。

### 5.5 模拟器验收指标

除 PnL 外，必须记录：

- 事件丢失、重复、乱序、断线、重连和深度重同步次数；
- 订单提交到 ACK、撤单到 ACK、未知状态持续时间；
- 队列等待时间、成交前队列量、部分成交比例；
- 1s/5s/30s markout、逆向选择成本和流动性坍塌损失；
- Post-only 拒单、撤单竞态成交、对账差异；
- 每标的及组合的最大持仓、保证金占用和清算缓冲；
- 同一事件流重复回放的一致性差异。

## 6. 策略方案：下一轮实施边界

### 6.1 固定锚点下的残差状态

锚点 A 不变，但新增可交易状态：

- residual：合约价格相对 A 的偏离；
- residual_velocity：偏离扩大或收敛速度；
- residual_duration：偏离持续时间；
- time_to_equity_open：距离股票开盘的时间；
- mark/index gap、盘口深度和资金费成本；
- 当前持仓是否处于“逆残差扩大方向”。### 6.2 风险状态机

采用 TRADING → CAUTION → REDUCE_ONLY → HALT，恢复使用滞回和冷却：

- TRADING：允许小规模正常 Maker 交易。
- CAUTION：减少新增风险，要求更高边际和更强回归证据。
- REDUCE_ONLY：取消增仓单，只允许降低已有仓位。
- HALT：停止新订单，等待数据、锚点或账户状态恢复。

Anchor residual 变大不能简单理解为更好的收益机会。残差越大时，新增仓位应先缩小；只有出现残差收敛速度、订单流和成交后 markout 同时改善，才允许恢复规模。

### 6.3 动态资金

M6 改为三层约束：

1. 组合风险预算：控制总名义、净方向和相关集中度。
2. 标的目标预算：根据波动、残差、资金费、深度和尾部状态分配。
3. 成交后硬边界：动态目标下降时立即取消增仓意图；实际仓位超过目标时强制进入减仓流程。

动态资金加入：

- 最小权重变化阈值；
- 最小仓位变化阈值；
- 再平衡冷却时间；
- 风险上升快速收缩；
- 风险下降慢速恢复；
- 不合格标的的新风险权重为零，不能用最低权重继续制造风险。

### 6.4 减仓和极端生存

- 严格 Maker-only 的正常减仓必须使用正确的非交叉报价方向。
- 资金费前、股票开盘前和硬风险突破时，必须有明确的退出可执行性检查。
- 如果坚持绝不吃单，系统必须把“无法及时退出”作为显式风险，而不是默认为已保护。
- 建议把 emergency flatten 设计成独立执行策略：正常交易不使用，只有硬风险、账户风险或清算风险触发；其成本单独统计，不混入普通策略收益。

## 7. 下一轮实验规则

下一轮不得单独跑某一个方法，必须完整并行：

- F1_m1、F2_m2、F3_m3、F4_m4、F5_m5、F6_m6；
- R1_m1、R2_m2、R3_m3、R4_m4、R5_m5、R6_m6；
- 仍使用同一公共行情、FX、时间轴和故障注入结果；
- 新输出目录独立保留，不能覆盖 M6 第一版；
- 模拟器修复和策略修复必须分别标记，不能把两类收益变化合并解释。

验收顺序：

1. 先通过 GNU 编译、单元测试、确定性回放和故障注入测试。
2. 再用修复后的模拟器跑完整 M1–M6。
3. 再比较策略改动前后；不得用旧模拟器结果与新模拟器结果直接排名。
4. 只有在保守成交、断线、深度坍塌和单边锚点偏离情景下仍改善，才保留策略改动。

## 8. 待讨论决策

### 决策 A：极端退出

推荐：正常交易 Maker-only；硬风险状态允许独立 emergency flatten，并单独统计成本。若不采用，必须接受极端情况下无法退出的尾部风险。

### 决策 B：锚点残差处理

推荐：锚点继续固定，但残差超过压力区后只减仓，不新增；恢复需要残差收敛和流动性恢复的联合证据。

### 决策 C：实施顺序

推荐：先修模拟器时间/盘口/订单生命周期，再修策略仓位和锚点风险；否则新旧结果无法归因。

## 9. 后续轮次记录模板

### Round N

- 日期：
- 方案目标：
- 模拟器改动：
- 策略改动：
- 不可变原则检查：
- 单元测试/回放测试：
- 完整矩阵实验目录：
- 结果：
- 保留项：
- 淘汰项：
- 尚未解决问题：
- 下一轮需要讨论的决策：

## 10. Round 2：现实约束与“最优解”的定义

### 10.1 本轮目标与边界

- 模拟器目标：复现“如果同一订单在真实 Binance 到达，会发生什么”，不负责提高策略收益。
- 策略目标：在模拟器给定的真实执行分布下，先保证生存，再最大化长期净增长。
- S2 是模拟器版本，不改变 M1–M6 策略规则；策略新增模块进入 M7，M6 永久保留作为动态资金第一版。
- 新实验仍必须并行跑 F1_m1…F7_m7 与 R1_m1…R7_m7；不得只跑 M7。
- 本轮只确定数学模型和实现契约，不改代码，不重启实验。

“最优”定义为词典序优化，而非单一加权分数：

\[
\text{第一层：满足所有硬安全约束；}\qquad
\text{第二层：在可行域内最大化最坏分布下的长期净增长。}
\]

非平稳、部分可观测市场中无法声称永久全局最优。本项目追求的是每个决策时点的鲁棒滚动最优，并通过保守回放、压力情景和持续校准逼近实盘最优。

### 10.2 现实资料带来的硬结论

- Binance diff-depth 只有在“pu == previous_u”时才连续；断链必须重新初始化本地簿。
- 普通深度快照不包含 RPI 流动性；显示盘口不等于全部可成交流动性。
- GTX/Post-only 必须在订单抵达交易所时按当时盘口判断，而非本地决策时判断。
- 修改限价单会重新排到撮合队尾；部分成交单的某些修改会导致取消。
- 真实执行必须同时建模 feed latency、order latency、队列位置与成交后的逆向选择。
- “成交概率高”与“成交后收益好”通常冲突，不能把 fill rate 当执行质量。

## 11. S2 模拟器数学模型（与策略完全独立）

### 11.1 单一离散事件时钟

每个市场包封装为：

\[
e_k=(t_k^{ex},t_k^{tx},t_k^{recv},seq_k,feed_k,payload_k)
\]

每个订单动作封装为：

\[
o_j=(t_j^{dec},t_j^{send},t_j^{arr},t_j^{ack},clientId_j,payload_j)
\]

- “t_ex”：交易所事件时间；“t_tx”：交易所事务时间；“t_recv”：本机单调时钟收包时间。
- “t_dec/send/arr/ack”：决策、发送、交易所抵达、确认到达本机的时间。
- 仿真器只按单一 sim_time 最小堆推进；同一时间以事件类别和 ingress_sequence 稳定排序。
- 禁止用 trade.event_time 与本地 arrival_time 直接比较。
- 实盘采集同时保存 wall clock 与 monotonic clock；回放使用记录的间隔，不使用回放机器的 now()。
- F/R 全 ledger 消费同一个不可变事件日志和同一组随机数种子（common random numbers）。

### 11.2 本地订单簿与数据可信状态

每标的维护 \(B_t=(B_t^{bid},B_t^{ask},u_t)\) 以及状态：

\[
D_t\in\{\text{SYNCING},\text{LIVE},\text{GAPPED},\text{RECONCILING},\text{HALTED}\}.
\]

- 先缓存 diff-depth，再取 REST snapshot，再从覆盖 lastUpdateId 的事件开始接续。
- 每个新事件必须满足序列连续；价格档数量是绝对量，零数量删除。
- gap、交叉簿、过期、重复冲突只冻结受影响标的；账户状态未知才全局 HALT。
- 冻结时取消增仓意图、停止模拟新成交；重建深度并完成订单/仓位对账后，以滞回恢复。
- 事件队列溢出必须在发生时触发 freeze/halt，不能等无限运行结束才报告。
- RPI/隐藏流动性作为不可观测量处理，不允许用普通 L2 快照假定“看见了全部队列”。

### 11.3 L2 队列与成交区间

订单在价格 \(p\) 抵达时，前方队列不是常数，而是区间：

\[
Q^{ahead}_{0}(p)\in[\rho_L Q^{disp}_{arr}(p),\rho_U Q^{disp}_{arr}(p)].
\]

对同价位显示量减少 \(\Delta Q^-_t\)，拆为可观测主动成交 \(V_t\) 与无法归因的撤单/修正 \(C_t\)：

\[
Q^{ahead}_{t+}= \max\{0,Q^{ahead}_t-V_t-\eta_t C_t\},\quad \eta_t\in[\eta_L,\eta_U].
\]

- 主动成交优先消耗前方队列；无法确定撤单发生在我方前还是后时，保留上下界。
- 保守主结果使用不利端参数；中性与乐观只作敏感性报告，不参与上线判定。
- 当穿价成交时必须视为 adverse fill 候选；触价但未穿价只能在前方队列耗尽后部分成交。
- 聚合 100ms 深度包内无法恢复精确先后顺序时，枚举所有合法顺序并取最不利可行结果。
- 自身挂单占该档显示量或成交量超过阈值时，历史回放的“小订单不影响市场”假设失效：样本标记 invalid 或叠加冲击模型。
- \(\rho,\eta\) 和隐藏流动性参数必须由实盘 shadow order/小额探针校准，不得凭经验固定。

### 11.4 成交后的逆向选择

每笔被动成交 \(f\) 记录方向化 markout：

\[
MO_f(h)=s_f\,[m_{t_f+h}-p_f],\qquad h\in\{1s,5s,30s,open\},
\]

其中买入 \(s_f=+1\)，卖出 \(s_f=-1\)。负值表示成交后价格向不利方向移动。

仿真验收比较联合分布：

\[
(\Pr[\text{fill}],fill\ latency,partial\ ratio,MO(1s,5s,30s),cancel\ race).
\]

只有成交率接近、markout 却过度乐观的模型仍然是不合格模型。

### 11.5 订单生命周期、延迟与交易所规则

订单状态机扩展为：

\[
Intent\rightarrow PendingSubmit\rightarrow PendingAck\rightarrow
\{New,PartFilled,Filled,Rejected,Unknown\}
\]

以及 PendingCancel、Canceled、Expired、ExpiredInMatch、Liquidation/ADL。

- submit ACK、私有流更新和 REST 查询是三个独立信息源；超时不等于拒单，而是 Unknown。
- Unknown 状态禁止重复制造方向风险，先按 clientOrderId/orderId 对账。
- Post-only 在 \(t^{arr}\) 的盘口上检查；取消在 cancel-arrival 前仍可能成交。
- amend 视为失去原队列优先级；filters、tickSize、stepSize、notional、percent-price 和限频均来自版本化 exchangeInfo。
- 延迟不再是 50/100ms 常数，使用条件经验分布：
\[
L\sim F_{endpoint,feed,load,hour}(l),
\]
并保留相关性、长尾、超时和重试；压力情景直接使用高分位与断线簇。
- API 限频、WebSocket 重连、ACK 丢失、磁盘阻塞、进程退出和机器暂停均进入故障注入。

### 11.6 账户、资金费、保证金和极端事件

每个时点计算：

\[
Equity_t=Wallet_t+UPnL_t,\qquad
Buffer_t=Equity_t-MM_t-Reserved_t.
\]

- UPnL 使用交易所 Mark Price；成交盈亏使用真实成交价；两者不能混用。
- \(MM_t\) 按每标的当前风险限额/杠杆档位做分段函数，并包含跨仓组合耦合。
- 资金费在真实结算时点按持仓和最终 funding rate 扣付；手续费按 maker/taker 与实际成交量计算。
- 模拟强平触发、强平费用、保险基金/ADL 风险情景，报告最小 Buffer/Equity。
- 压力情景至少包含锚点单边偏离、深度骤降、价差跳宽、延迟长尾、断线、资金费跳变、多标的相关冲击。
- 无限运行必须有 supervisor、heartbeat、退出码和原子 checkpoint；意外退出后能判断原因并从一致状态恢复。

## 12. M7 策略数学模型（S2 验收后才实现）

### 12.1 固定锚点与可变状态

锚点 \(A_i\) 永远固定。对标的 \(i\) 定义对数残差：

\[
x_{i,t}=\log(P_{i,t}/A_i).
\]

锚点数值不变，但“此刻是否适合交易该锚”是可变隐状态：

\[
S_{i,t}\in\{N:\text{正常回归},C:\text{谨慎拉伸},R:\text{疑似重定价},B:\text{数据/市场破坏}\}.
\]

在状态 \(s\) 下使用带跳跃的状态切换 OU：

\[
dx_t=\kappa_s(\mu_s-x_t)dt+\sigma_s dW_t+dJ_t^{(s)},\qquad
\Pr(S_{t+dt}=r|S_t=s)=q_{sr}dt.
\]

- 正常状态约束 \(\mu_N=0,\kappa_N>0\)，体现固定收盘锚回归。
- R 状态允许弱回归、漂移或跳跃强度上升，但绝不修改 \(A\)。
- 观测包含残差、速度、持续时间、OFI、深度、spread、mark/index、funding、成交后 markout、距股票开盘时间。
- 用可解释的在线 Bayesian/HMM filter 得到 \(\pi_t(s)\)；禁止未来数据、整段平滑和深度学习热路径。
- 参数使用滚动稳健估计、指数遗忘和置信区间；样本不足进入 CAUTION，不补造确定性。

### 12.2 回归收益不是“偏离越大越好”

正常 OU 条件均值仅给出：

\[
\mathbb E[x_{t+h}|x_t,S=N]=x_t e^{-\kappa_Nh}.
\]

真正需要的是在持仓截止 \(H\) 前到达退出带的首达概率与尾部损失：

\[
p_{conv}=\Pr(\tau_{exit}\le H),\quad
\tau_{exit}=\inf\{u:x_{t+u}\in\mathcal E\}.
\]

大残差同时提高潜在回归收益与“已经重定价”的后验概率。入场必须满足最坏情形净边际：

\[
Edge^{robust}=\inf_{\theta\in\Theta_t}
\mathbb E_\theta[PnL_{net}\mid fill,action] > 0,
\]

其中 \(PnL_{net}\) 显式扣除手续费、资金费、成交逆向选择、退出成本、未成交机会成本和跳跃尾损。

### 12.3 执行决策：联合优化价格、数量和等待

动作不是简单方向，而是：

\[
a_t=(side,priceLevel,qty,ttl,cancel/hold,reduce).
\]

每个候选 Maker 动作计算：

\[
V(a)=p_{fill}(a)[G_{conv}(a)-Fee-Funding-AS(a)-ExitCost]
-(1-p_{fill}(a))OpportunityLoss-Risk(a).
\]

- \(p_{fill}\) 来自 S2 的状态依赖队列/首达模型，不使用固定概率。
- AS 来自同状态下 1s/5s/30s 条件 markout 分布。
- 若动作只提高成交概率但使成交后净值下降，则拒绝。
- 线性摩擦天然产生 no-trade band；残差刚覆盖 spread/fee 不足以入场。
- 每次市场事件后做有限候选集的滚动优化，保留上一订单可避免无意义撤挂。
- 正常 Maker 减多仓应在 ask 卖，减空仓应在 bid 买；当前反向实现必须先修复并单测。

### 12.4 生存优先的组合优化

第一层硬约束：

\[
\Pr_\theta(Buffer_{t:t+H}\le0)\le\varepsilon,\quad
CVaR_\alpha(DD_H)\le D_{max},\quad |q_iP_i|\le N_i^{hard}.
\]

并限制 gross、net、相关因子暴露、单标的集中度、流动性参与率、订单速率和距开盘剩余风险。

第二层在上述可行域内求：

\[
\max_{a,w}\inf_{\mathbb P\in\mathcal U_t}
\mathbb E_\mathbb P\left[\sum_{u=t}^{t+H}\log\left(1+\frac{\Delta W_u}{W_u}\right)\right]-\lambda TO(a,w).
\]

- \(\mathcal U_t\) 是由参数置信区间、压力情景和经验分布构成的 ambiguity set。
- 动态资金不再做纯 inverse-risk；使用情景损益矩阵求解 robust CVaR/增长问题。
- 风险上升立即收缩，风险下降缓慢恢复；权重和仓位均有 deadband、滞回和 cooldown。
- 新目标低于实际仓位时：先取消增仓单，再进入硬减仓，未回到上限前权重不得恢复。
- 名义仓位按当前 Mark、合约乘数和 filters 计算，不能按固定 Anchor 价格代替。

### 12.5 状态机与极端退出

策略状态保持：

\[
TRADING\rightarrow CAUTION\rightarrow REDUCE\_ONLY\rightarrow HALT.
\]

- TRADING：仅在 \(Edge^{robust}>0\) 且风险约束有余量时增加风险。
- CAUTION：提高门槛、缩短 TTL、降低目标，禁止对极端残差机械抄底/摸顶。
- REDUCE_ONLY：实际仓位超过硬目标、重定价概率过高、临近开盘/资金费或保证金恶化时触发。
- HALT：数据、订单、账户或锚点状态未知。
- 恢复必须同时满足残差收敛、流动性恢复、数据连续、订单对账和冷却完成。
- emergency flatten 仍作为独立待决安全层：普通收益完全不使用 taker；若启用，仅硬风险可触发并单独统计。

## 13. S2 与 M7 的验证设计

### 13.1 S2 模拟器验收

- 单元性质：时间单调、状态转换合法、仓位守恒、现金守恒、同种子确定性。
- Binance 规则：depth 重建、GTX 抵达拒单、amend 丢队列、cancel-race、部分成交、Unknown 对账。
- 校准：按标的/时段比较 shadow/live 的 ACK 延迟、fill、partial、queue wait 和 markout 联合分布。
- 保守性：主结果不得系统性高估 fill 或低估负 markout；区间覆盖率必须报告。
- 可靠性：进程异常退出、事件丢失、磁盘阻塞、重连和 checkpoint 恢复均有自动测试。
- S2 通过前，不得用其结果决定 M7 优劣。

### 13.2 M7 策略验收与完整消融

同一 S2 事件流并行保留 F1_m1…F6_m6、R1_m1…R6_m6，并新增 F7_m7、R7_m7。M7 内部消融至少拆分：regime、robust_edge、hard_rebalance、portfolio_cvar、execution_value；每个消融也必须共享事件、延迟样本、故障样本和随机种子。

主判定使用配对差分，而不是各跑一次后比较绝对 PnL：

\[
\Delta Y_k=Y_k(M7)-Y_k(M6).
\]

在多日、多状态、多随机种子下报告置信区间。通过条件：

1. 正常市场净收益和收益/占用资金改善；
2. 极端情景的最大回撤、最小保证金缓冲和尾部 CVaR 不恶化；
3. 收益不是由更高名义、更高成交乐观度或更少故障样本造成；
4. 每标的均报告 PnL 分解、fill/markout、持仓、资金费、费用和状态占比；
5. 任一模块没有稳定增量价值则淘汰，但 M1–M6 历史版本仍保留。

## 14. Round 2 实施顺序

1. 先修复已确认的 Maker reduce-only 报价方向 bug，并加执行契约测试。
2. 实现 S2 单时钟、真实订单状态机、标的级重同步和实时 halt。
3. 实现动态 L2 队列区间、条件延迟、保证金/清算与 supervisor/checkpoint。
4. 用旧 M1–M6 完整矩阵验证 S2；此阶段不引入 M7。
5. S2 验收后实现 M7 regime、robust edge、硬再平衡和组合风险优化。
6. 跑 M1–M7 全矩阵、多日、多种子和故障压力测试，保留全部版本。

## 15. Round 2 尚未解决的决策

- 极端退出：是否允许只在保证金/清算硬风险下使用 taker emergency flatten。
- 校准数据：是否允许用极小真实订单做 shadow/probe；若不允许，只能输出更宽、更保守的成交区间。
- 组合硬阈值：\(\varepsilon,\alpha,D_{max}\)、最小保证金缓冲和单标的集中度需要在 1 万元人民币账户尺度下确定。
- M7 的首个实现应保持可解释 HMM/稳健滤波，不引入神经网络。

## 16. Round 2 主要资料

- [Binance：本地订单簿同步](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/websocket-market-streams/How-to-manage-a-local-order-book-correctly)
- [Binance：USDⓈ-M WebSocket 交易与 GTX/改单](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-api/trade)
- [Binance：市场流与 diff-depth/RPI](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/public)
- [Binance：ADL Risk](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data)
- [HftBacktest：延迟、L2/L3 与队列回放](https://hftbacktest.readthedocs.io/en/py-v2.1.0/)
- [NautilusTrader：同核心组件回测](https://nautilustrader.io/docs/latest/concepts/backtesting/)
- [Fill probabilities under state-dependent order flow, 2026](https://arxiv.org/abs/2403.02572)
- [Fill probability vs post-fill return, 2025](https://arxiv.org/abs/2502.18625)
- [Market simulation under adverse selection, 2024](https://arxiv.org/abs/2409.12721)
- [Optimal trading of microstructure mean reversion, 2026](https://arxiv.org/abs/2608.00885)
- [Model-free excursion analysis of mean reversion, 2026 version](https://arxiv.org/abs/2011.02870)
- [Optimal mean reversion with costs and stop-loss](https://arxiv.org/abs/1411.5062)
- [Microstructural FIFO LOB stochastic control](https://arxiv.org/abs/1705.01446)

## 17. Round 3：日志、归因与评价体系

### 17.1 本轮目标

Round 3 不改变固定锚点原则，也不提前实现 M7。目标是建立四个彼此独立、又能用因果 ID 连接的系统：

1. Simulator Truth：市场、交易所、网络和账户发生了什么。
2. Strategy Intent：策略看到了什么、计算了什么、为何选择或拒绝某动作。
3. Economic Ledger：每一分钱、每一单位仓位如何变化。
4. Evaluation：在共同机会集上，哪个方法在何种状态下真正更好。

必须禁止以下混淆：

- 模拟器造成的成交变化，不能归因成策略 alpha。
- 内部门禁不能记为 exchange reject。
- 未成交不能只写“没有订单”，必须有具体 reason code 和候选动作价值。
- 实时近窗指标不能替代全程累计指标。
- 单个总分不能让高收益抵消强平、数据损坏或不可恢复状态。

### 17.2 当前实现审计结论

- AuditRecord 只有 sequence、单时间戳、kind、symbol、reason、correlation_id，无法表达完整因果链与多时钟。
- PaperRecord 只记录 placed/canceled/fill/funding/rebalance 等少数结果，没有 feature、候选动作、门禁分解、ACK、队列变化和状态转换。
- rejected_entries 同时承载“无可接受信号”和本地 maker 校验失败，命名与口径错误。
- metrics.json 的 history 只保留 900 点；长跑会丢弃早期 equity peak，最大回撤和 Sharpe 只反映近窗。
- 当前 Sharpe/Sortino 对 30 秒 PnL 增量直接按全年秒数年化；序列相关、重叠持仓和非独立样本会造成虚高。
- run-manifest 没有 git commit、dirty diff hash、数据 hash、配置 hash、schema 版本、随机种子、退出原因和退出码。
- records 写盘允许 drop，且不能立刻在 ledger 级冻结；这不满足审计账本要求。

## 18. 四层可观测架构

### 18.1 A 层：不可变原始事实

按来源分别记录 append-only 原始流：

- market_raw：depth、bookTicker、aggTrade、mark/index、funding；
- account_raw：ORDER_TRADE_UPDATE、TRADE_LITE、余额、仓位、ADL、REST reconciliation；
- reference_raw：官方收盘锚、交易日历、FX、exchangeInfo、费率与杠杆档位；
- system_raw：连接、DNS/代理、限频、队列、磁盘、进程、时钟偏移和异常。

原始记录必须包含 raw payload 或其内容寻址引用、source、connection_id、exchange sequence、exchange event/transaction time、local receive wall/monotonic time、ingress sequence、CRC/hash。

### 18.2 B 层：标准化领域事件

统一 Envelope：

\[
E=(schema,run,source,eventId,parentId,traceId,seq,times,entity,type,payloadHash,payload).
\]

所有领域事件使用整数 tick/quantity 和记账币最小单位。显示用小数是派生值，不能成为账本真值。

关键 ID：

- experiment_id、run_id、ledger_id、strategy_version、simulator_version；
- market_event_id、decision_id、candidate_id、risk_eval_id；
- intent_id、client_order_id、exchange_order_id、fill_id；
- position_lot_id、rebalance_id、incident_id、checkpoint_id；
- trace_id 和 parent_event_id。

同一市场事实只写一次；各 M1–Mxx ledger 通过 market_event_id 引用，避免复制后发生漂移。

### 18.3 C 层：经济账本与派生事实

经济账本必须双轨：

- Accounting Ledger：严格可加总、可对账的现金/仓位事实。
- Model Attribution：依赖策略模型的预测贡献，必须标明 model_version，不能覆盖会计事实。

每个 fill 创建不可变成交 lot。账户聚合仓位可按 Binance 口径维护，但分析层同时保留 lot 生命周期。任何时点满足：

\[
Equity=Wallet+UPnL,\qquad
\Delta Equity=TradingPnL+Funding-Fees-LiquidationCost+Transfer.
\]

持仓期间使用精确分解：

\[
q\Delta Mark=q\Delta Index+q\Delta(Mark-Index).
\]

进一步把 \(q\Delta Index\) 按路径事实分为 residual 收敛段与扩散段；模型归因另分为：

\[
q\Delta Index=q\,\mathbb E_t[\Delta Index]+q(\Delta Index-\mathbb E_t[\Delta Index]).
\]

前者是预测贡献，后者是意外冲击；两者都不能和成交执行成本混算。

### 18.4 D 层：在线指标与离线研究

- live_snapshot.json：只服务监控，允许近窗统计。
- cumulative_state.json：全程可合并统计量、历史峰值、全程回撤和计数，永不因 ring buffer 丢失。
- equity_curve：独立分段时序文件，不嵌入一个不断膨胀的 metrics.json。
- metrics_final：结束/恢复点生成全程结果。
- metric_cube：method × symbol × session × regime × direction × residual_bucket × fill_type。
- comparison_report：配对差分、置信区间、支配关系和失败原因。

JSONL 可保留为审计格式；大规模分析可异步压缩为 Parquet。压缩层不得成为唯一事实源。

## 19. 端到端因果日志

### 19.1 一次决策必须能完整重放

因果链固定为：

\[
MarketEvent\rightarrow FeatureSnapshot\rightarrow RegimeEstimate
\rightarrow CandidateSet\rightarrow RiskEvaluation\rightarrow Decision
\rightarrow OrderIntent\rightarrow Send\rightarrow ExchangeAck
\rightarrow QueueEvolution\rightarrow Fill/Cancel\rightarrow PnL.
\]

FeatureSnapshot 至少记录 anchor residual/velocity/duration、bid/ask/L2、mark/index、OFI、波动、funding、距开盘时间、当前仓位、目标仓位、保证金缓冲、数据年龄与质量。

CandidateAction 每个候选都记录：

- side、price level、quantity、TTL、reduce_only；
- predicted fill probability/time-to-fill；
- convergence gain、fee、funding、adverse-selection、exit cost、risk cost；
- robust edge 下界/中位/上界；
- 每条硬约束的 slack；
- chosen=false 时的精确淘汰原因。

即使最终 NO_ACTION，也必须能回答“有哪些候选、哪一项约束或哪一项成本使其失败”。

### 19.2 Reason code 规范

禁止只写自由文本 detail。使用稳定枚举和可选上下文：

- DATA.DEPTH_GAP、DATA.MARK_STALE、DATA.FX_STALE；
- ANCHOR.MISSING、ANCHOR.NOT_FINAL、ANCHOR.AGE_LIMIT；
- SIGNAL.EDGE_BELOW、SIGNAL.REGIME_REPRICE、SIGNAL.CONFIDENCE_LOW；
- RISK.POSITION_CAP、RISK.CVAR、RISK.MARGIN_BUFFER、RISK.CONCENTRATION；
- EXEC.POST_ONLY_CROSS、EXEC.QUEUE_TOO_LONG、EXEC.CANCEL_RACE；
- EXCHANGE.REJECT_FILTER、EXCHANGE.REJECT_RATE_LIMIT、EXCHANGE.UNKNOWN；
- SYSTEM.WRITER_BACKPRESSURE、SYSTEM.CLOCK_JUMP、SYSTEM.PROCESS_EXIT。

每个 code 单独计数；“策略拒绝、本地校验、交易所拒绝、模拟器拒绝”必须四分。

### 19.3 订单与队列日志

每个订单记录全部状态迁移及前后状态：

- intent_created、submit_enqueued、wire_sent、exchange_arrived；
- ack_new、partial_fill、fill、cancel_requested、cancel_arrived、cancel_ack；
- rejected、expired、expired_in_match、unknown、reconciled；
- transition_from、transition_to、trigger_event_id、latency component。

QueueEvent 记录 price、displayed_before/after、trade_qty、cancel_qty、estimated_ahead_low/base/high、own_remaining、fill_low/base/high 和归因假设。

Post-only reject 使用订单抵达时盘口快照；cancel-race 必须关联 cancel_request 与导致成交的 market_event。

### 19.4 PnL 与风险日志

每次仓位、mark、funding、fee、margin 变化都写 LedgerEntry：

- amount、currency、component、symbol、fill/lot/order/decision ID；
- realized/unrealized、before/after balance、before/after position；
- mark/index/anchor、margin tier、maintenance margin、buffer；
- exact_accounting=true/false。

PnL component 至少包含：

- convergence_aligned、residual_divergence、mark_index_basis；
- entry_execution、exit_execution、spread_capture、adverse_markout；
- maker_fee、taker_fee、funding_regular、funding_special；
- emergency_exit、liquidation、rounding/filter、FX；
- unexplained_reconciliation。

所有 component 必须精确加总到总 equity 变化；unexplained 不得被静默归零。

### 19.5 系统可靠性和存储保证

- audit/accounting writer 禁止 drop；背压超过阈值时冻结新增风险。
- market raw 队列溢出立即标记具体 symbol gap；不能只在运行结束检查 AtomicU64。
- 文件分段、原子 rename、checksum、尾部截断恢复和 schema migration 必须测试。
- 每个 segment 记录前一段 hash，形成可验证链；checkpoint 保存最后 committed sequence/hash。
- supervisor 记录启动命令、父进程、心跳、signal/exception、退出码、stderr 尾部和恢复结果。
- 日志中 API key、secret、签名、代理凭证永不落盘；client order id 可以落盘。
- 运行 manifest 固化 git commit、dirty_patch_hash、GNU toolchain、binary hash、config hash、data hash、random seed、时区和依赖版本。
- 任何恢复都生成新 run_attempt_id，但保持同一 experiment_id，禁止伪装成从未中断。

## 20. 指标体系：先验分层，不做单一总分

### 20.1 Level 0：数据与模拟器合格门

任一项失败，该 run 不进入策略排名：

- event loss/duplicate/conflict、sequence gap、crossed book、clock reversal；
- raw-to-domain 完整率、ledger reconciliation error；
- order state illegal transition、unknown duration；
- writer drop、checkpoint corruption、nondeterministic replay hash mismatch；
- fee/funding/filter/margin rule version missing。

模拟器准确度按真实 shadow/live 联合分布校准：

- latency：p50/p90/p99/p99.9、Wasserstein distance；
- fill：Brier score、log loss、reliability curve、ECE；
- time-to-fill：生存曲线、censoring-aware integrated Brier score；
- queue：预测区间覆盖率和宽度；
- markout：1s/5s/30s 分布距离与尾部分位误差；
- cancel race、partial fill、post-only reject 的频率误差。

### 20.2 Level 1：生存与尾部风险

这是策略排名第一层，不能被收益抵消：

- liquidation/ADL 次数、risk-of-ruin 上界；
- minimum margin buffer、buffer/Equity 的 p1/p5；
- max drawdown、drawdown duration、time under water、recovery time；
- VaR/CVaR/expected shortfall、worst session、worst excursion；
- jump loss、gap loss、disconnect loss、emergency-exit cost；
- 压力场景损失：残差 5%/10%/20%、深度 -50%/-90%、延迟 p99.9、相关性趋近 1；
- 超硬仓位时间、目标超调面积：
\[
OvershootArea_i=\int (|N_{i,t}|-N^{hard}_{i,t})_+dt.
\]

### 20.3 Level 2：经济收益与资本效率

- realized、unrealized、gross、net PnL 及完整 component；
- total return、session return、return on allocated capital；
- return on average/peak margin、PnL per gross-notional-hour；
- profit factor、payoff ratio、win/loss excursion、expectancy；
- fee drag、funding drag、execution drag、adverse-selection drag；
- exposure time、gross/net exposure、turnover、capacity/participation；
- Calmar、Omega、Ulcer index；Sharpe/Sortino 仅作辅助，不作唯一结论。
- 好市场收益定义为“有效机会强度上分位且数据/流动性正常”条件下的净收益与机会捕获率，而不是事后挑选上涨行情。

### 20.4 Level 3：信号与状态模型

- canonical opportunity count、action coverage、entry frequency；
- 按 reason code 的拒绝漏斗；
- \(p_{conv}\) 的 Brier/log loss、校准斜率/截距、ECE；
- predicted edge 与 realized net edge 的 MAE、bias、rank correlation；
- half-life/first-passage 预测误差；
- regime 后验稳定性、切换次数、停留时间、各状态条件 PnL；
- residual bucket、方向、距开盘/资金费时间分桶表现；
- false mean-reversion entry：入场后先触发 stop/risk state 而未进入退出带。

### 20.5 Level 4：执行质量

- submit/ACK/cancel/reconcile latency 全分位；
- fill rate 必须按价格档、队列分位、状态、方向条件化；
- time-to-first-fill、time-to-complete、partial ratio、unfilled expiry；
- implementation shortfall：
\[
IS=s(P_{fill}-M_{decision}),
\]
买入 \(s=+1\)，卖出 \(s=-1\)，正值为成本。
- price improvement、effective spread、realized spread；
- 100ms/1s/5s/30s/open markout 与 adverse-fill ratio；
- queue wait、queue ahead depletion、cancel-to-fill race；
- quote age、reprice count、cancel/order ratio、amend/order ratio；
- maker/taker ratio、post-only reject、filter reject、rate-limit reject；
- reduce-only 完成率、达到硬目标耗时、退出 shortfall。

### 20.6 Level 5：组合和动态资金

- target vs actual notional、tracking error、overshoot duration/area；
- rebalance count、有效变更率、无效 churn、换手成本；
- 单标的权重、HHI、最大集中度、边际 CVaR 贡献；
- gross/net、方向偏置、共同因子暴露、相关冲击贡献；
- 每单位风险预算的净收益、风险预算利用率；
- 风险上升收缩延迟、风险下降恢复延迟；
- 资金从被降权标的转出后带来的增量 PnL 与避免损失；
- dynamic capital 相对固定等权的配对增量，而非只看 M6/M7 自身绝对收益。

### 20.7 Level 6：运行与运维

- uptime、feed availability、symbol freeze/HALT 时间占比；
- reconnect/resync/reconcile 次数与恢复时间；
- event/decision/order/log throughput，队列 high-water mark；
- CPU、memory、disk bytes、fsync latency；
- heartbeat miss、process restart、checkpoint recovery point objective；
- 指标生成延迟和报告完整率。

## 21. 方法优劣的统计比较

### 21.1 共同机会集与独立样本

所有方法在 canonical decision epochs 上评估。即使某方法不交易，也要保存其候选动作和 counterfactual value，避免只比较各自成交样本造成选择偏差。

统计基本单位优先使用：

- 一个完整股票闭盘 session；
- 一个独立 anchor-residual excursion；
- 一个预注册压力情景 × seed。

禁止把每个 tick、每次刷新或重叠 30 秒 PnL 当独立样本。现有“30 个样本即可 Sharpe”的规则淘汰。

### 21.2 配对估计

对方法 \(a,b\)，在完全相同市场事件、延迟样本、故障样本与随机种子上计算：

\[
\Delta Y_k=Y_{a,k}-Y_{b,k}.
\]

报告 mean/median、Hodges-Lehmann effect、block-bootstrap 置信区间、胜率和：

\[
P(\Delta Y>0),\qquad P(\Delta Risk\le0).
\]

时间相关性使用 session/excursion block bootstrap 或 HAC；普通 IID 标准误禁止用于高频序列。

### 21.3 多重试验和数据窥探

- manifest 预注册 primary metric、baseline、版本集合和淘汰门槛。
- Sharpe 使用自相关修正并报告置信区间；同时报告 Probabilistic Sharpe Ratio。
- 多版本、多参数搜索必须报告 Deflated Sharpe Ratio。
- 全方法族对基线使用 White Reality Check 或 Hansen SPA，控制“试得越多越容易偶然赢”。
- 调参窗口、验证窗口、最终锁箱窗口严格分离；锁箱失败不得回头改口径。
- 参数敏感性以稳定平台为优，不选择尖锐单点最优。

### 21.4 支配规则

方法 A 只有同时满足才可称优于 B：

1. 两者均通过 Level 0；
2. A 所有生存硬约束通过；
3. primary net-return/capital-efficiency 的配对增量置信下界大于预设最小经济效应；
4. CVaR、max drawdown、margin buffer 没有超过容忍度的恶化；
5. 改善不只来自一个标的、一个 session、一个方向或一个极端 fill；
6. 在成本、延迟、队列三组保守参数下结论方向稳定。

否则标记为 trade-off、inconclusive 或 rejected，不能只按净 PnL 排名。

## 22. 报告矩阵与深度归因输出

每次完整实验自动生成：

1. Executive survival sheet：每方法硬门是否通过、失败原因。
2. Method × Symbol 主表：全部收益、风险、执行、信号、资本指标。
3. Pairwise delta table：M2−M1、M3−M2…M7−M6 及 F/R 配对差。
4. Attribution waterfall：总 PnL 到 convergence/divergence/basis/execution/funding/fee/emergency。
5. Drawdown episodes：前 N 次回撤的起点、谷底、恢复、标的和因果事件。
6. Fill diagnostics：队列、成交概率、markout、取消竞态校准图。
7. Regime cube：状态 × 标的 × 方向 × residual bucket。
8. Rejection funnel：每级 gate 的机会数、拒绝数和潜在/避免损失。
9. Capital diagnostics：目标/实际、超调面积、集中度、边际 CVaR。
10. Reliability report：gap、drop、重连、冻结、退出、恢复与校验和。
11. Reproducibility report：commit/config/data/binary/hash/seed 与 replay hash。
12. Incident bundle：任一大亏、unknown order、对账差异可一键导出完整 trace。

每个表同时提供 aggregate、per-method、per-symbol、per-session。总表不得替代单标的结果。

## 23. Round 3 实施分期

### L1：先修口径，防止继续产生不可解释数据

- 拆分 strategy_block/local_validation/exchange_reject/simulator_reject。
- 修复全程累计回撤；近窗和 lifetime 指标分离。
- manifest 增加版本/hash/seed/exit metadata。
- 运行错误、退出来源、writer drop 立即落盘。
- 修复 Maker reduce-only 方向 bug。

### L2：事件溯源骨架

- 实现统一 Envelope、因果 ID、ReasonCode 和订单状态事件。
- 实现 Accounting Ledger、不变量校验和全链 trace。
- shared market facts 与 ledger decisions 分离引用。
- 分段写盘、checksum、checkpoint、恢复测试。

### L3：深度归因和指标

- candidate action、risk slack、queue evolution、markout、PnL component。
- metric cube、paired delta、block bootstrap、DSR/SPA。
- 自动生成每方法每标的报告和 incident bundle。

### L4：再进入 S2/M7

- 用新日志体系验证 S2 真实性。
- S2 合格后实现 M7；随后完整并行 M1–M7。
- 每轮保留旧输出、旧 schema reader 和 migration 记录。

## 24. Round 3 验收标准

- 任一 fill 可从原始市场包追溯到 feature、decision、order、queue、PnL。
- 任一 NO_ACTION 可给出候选动作、逐项成本和首个失败硬约束。
- 任一总 PnL 可精确分解，未解释差异为零或显式 reconciliation error。
- 任一指标可定位到原始事件集合、公式版本与聚合维度。
- 任一方法比较均使用同一机会集和配对样本。
- 长跑删除内存 history 后，lifetime max drawdown、peak equity 和累计统计不变。
- 重放两次，事件 hash、订单状态、账本和指标完全一致。
- 人为注入 drop/gap/clock jump/disk full/process kill 时，系统按契约 freeze/halt 并留下完整证据。
- F/R 若理论上只差一项配置，除此项外的共享输入 hash 必须一致。
- 报告必须覆盖全部方法、全部 7 个标的和全部预注册分层，不允许只汇报赢家。

## 25. Round 3 研究依据

- [Binance USDⓈ-M 市场流：aggTrade 为 100ms 聚合且不含 ADL/保险基金交易](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/market)
- [Binance ORDER_TRADE_UPDATE 状态与 STP](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/faq/stp-faq)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [OpenTelemetry Logs Data Model](https://opentelemetry.io/docs/specs/otel/logs/data-model/)
- [OpenTelemetry Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- [Lo：The Statistics of Sharpe Ratios](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=377260)
- [Bailey 与 López de Prado：Deflated Sharpe Ratio](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=2460551)
- [White：Reality Check for Data Snooping](https://users.ssc.wisc.edu/~behansen/718/White2000.pdf)
- [Hansen：Superior Predictive Ability Test](https://www.tandfonline.com/doi/abs/10.1198/073500105000000063)
- [Newey-West：HAC covariance](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=225071)

## 26. Round 4：数学最优解的边界与总决策

“最优”只允许指：在明确的信息集、动作集、校准样本、威胁集与计算预算内的可复现最优；不宣称对未知真实市场全局最优。无限跳跃、交易所停机、规则突变下不存在无条件存活保证。

采用词典序目标：

1. 数据、交易规则、账本和模拟器通过资格门；
2. 在预注册威胁集内满足生存与可执行硬约束；
3. 在可行域内最大化最坏分布下的长期净对数增长；
4. 主目标近似相同时，依次选择更低 CVaR、更低换手、更简单且更稳定的策略；
5. 若最优动作相对 NO_ACTION 的鲁棒价值差未超过估计误差，必须 abstain。

Round 4 不创建小数版本。S2 仍是下一代模拟器；M7 仍是下一代策略。M6 永久保留为动态资金第一版，下一次正式实验仍为 M1–M7 全部并行。

## 27. S2 模拟器：两种真值模式必须分开

### 27.1 Factual Replay / Paper Truth

真实收到的 depth、trade、mark、index、funding、account 与 order update 是不可改写事实。对未观测到的撮合队列不伪造单点答案，而维护：
$Q_{ahead}(t) \in [Q_{low}(t),Q_{high}(t)]$。

每次深度变化按价格档位、方向、成交证据与可撤量，传播队列区间；每一笔自有订单同时生成 pessimistic/base/optimistic 三条一致轨迹。正式排名以 pessimistic 或预注册混合权重为主，不能事后挑路径。

自有下单仅在 exchange-arrival 时检查 GTX/Post-only；撤单与成交是 competing risks。任何 Unknown、断线、序列 gap、规则过期或账本不一致都冻结新增风险，直至 REST/stream 对账完成。

### 27.2 Generative Stress Truth

压力与反事实模式采用状态依赖的 marked Queue-Reactive Hawkes 模型。事件 $e=(type,side,level,size)$ 的强度为：

$\lambda_e(t)=softplus(\beta_e(s_t,B_t)+\sum_j\int\phi_{ej}(t-u,m_j)dN_j(u))$。

$B_t$ 包含多档队列、spread、imbalance、mark-index basis、波动与 session phase；$m_j$ 表示订单大小。核矩阵必须满足稳定/非爆炸约束，并通过 time-rescaling、残差独立性、事件类型/大小分布和压力期覆盖检验。

S2 不用生成模型替换事实回放。生成模型只用于：

- 未见压力情景、故障注入与反事实冲击；
- 估计取消竞态、流动性枯竭和自激订单流的尾部；
- 检查策略是否只利用了某个模拟器的可识别伪影。

深度 Queue-Reactive 模型只能作为 challenger。只有在锁箱数据上同时改善条件分布、尾部、路径统计和策略排名稳定性，且不破坏可审计性时，才可用于 stress ensemble；不得进入策略热路径。

### 27.3 模拟器校准不是“拟合损失最小”

校准目标是多目标约束：
$\min_\theta \sum_k w_k d_k(T_k(sim_\theta),T_k(real))$，
其中必须含 inter-arrival、size、spread、depth、imbalance、duration、return、volatility clustering、impact、markout、fill、cancel race 与极端流动性指标。

每个距离都报告样本外置信区间。若某关键统计不合格，S2 整体为未认证，不能用其 PnL 给策略升级背书。

## 28. M7 隐状态动力学：固定锚点不变，信念可变

对标的 $i$ 定义固定官方收盘锚点 $A_i$，以及
$x_i(t)=\log(P_{index,i}(t)/A_i)$。$A_i$ 在闭盘 session 内严格常数；模型只能改变“未来是否回归”的信念，不能移动锚点。

Index residual 驱动经济状态；可交易 mid、mark 分别通过观测方程加入 contract-index basis 与 mark-index basis。执行收益按真实 bid/ask/fill 结算，保证“信号价格、可交易价格、清算价格”不混为一个价格。

潜在状态 $S_i(t)\in\{Normal,Caution,Reprice,Broken\}$，采用显式持续时间的半马尔可夫模型，避免普通 HMM 的几何持续时间假设。给定状态：

$dx_i=[\kappa_s(\mu_s-x_i)+\beta_s^\top f_i]dt+\sigma_s dW_i+dJ_i$。

$J_i$ 是状态依赖的双向复合跳跃；跨标的冲击由低维共同因子和稀疏残差相关连接。Normal 中约束 $\kappa_s>0$；Reprice/Broken 允许弱回归、零回归或远离锚点。$\mu_s$ 是残差条件均衡项，不是新锚点。

观测向量只含决策时已到达的信息：residual 路径、mark-index basis、spread/depth/imbalance、订单流、波动、资金费、距股票开盘/资金费时间、数据健康和账户风险。禁止未来平滑、事后 session 分类和用成交结果回填当时特征。

## 29. 因果在线推断

M7 使用 Rao-Blackwellized 粒子/IMM 过滤半马尔可夫状态，并叠加 Bayesian Online Changepoint Detection：

$b_t(s,r,\theta)=P(S_t=s,runlength_t=r,\theta\mid\mathcal F_t)$。

参数采用分层贝叶斯收缩：全市场先验 → A/HK session 类别 → 单标的后验，解决 7 个标的单独样本不足。连续时间 OU 使用不规则间隔的精确转移密度，不把 tick 当等间隔样本。

在线只做过滤；离线可做平滑用于诊断，但平滑结果不得进入历史决策重放。后验有效样本量过低、变点概率过高或模型集合分歧过大时，状态直接进入 Caution/Broken 并缩减风险。

## 30. 从“偏离大”升级为首达概率与净边际价值

入场不再由 z-score 单独触发。对每个候选动作求联合事件：

$p_{conv}=P(\tau_{target}\le T,\tau_{stop}>T,no\ anchor\ break\mid\mathcal F_t,a)$。

其中 $\tau_{target}$ 是进入获利残差带的首达时间，$\tau_{stop}$ 是触及风险边界的首达时间。对 regime-switching jump-OU，离线用 backward Kolmogorov PIDE/动态规划生成网格，在线由后验混合和局部插值求值；网格外必须保守外推或 NO_ACTION。

候选动作的路径净收益为：

$G(a,\omega)=Convergence+SpreadCapture-ResidualDivergence-BasisMove-Fees-Funding-AdverseSelection-EmergencyExit-Rounding$。

成交不是独立伯努利。使用 fill、mid-price move、cancel 与 timeout 的 competing-risk 模型，并估计 $E[markout\mid fill,state,queue]$；否则高成交率会把逆向选择误认成优势。

定义鲁棒边际价值下界：

$LCB(a)=\inf_{Q\in U_t}E_Q[G(a)]-\lambda\sup_{Q\in U_t}CVaR_\alpha(-G(a))-model\_error(a)$。

仅当 $LCB(a)>0$、硬约束全部通过且相对 NO_ACTION 的价值差超过数值/统计误差时，才允许增加风险。

## 31. 因果 Wasserstein 鲁棒 POMDP

真实状态由市场状态、状态后验、订单/队列、仓位、现金、保证金、时钟和系统健康构成；动作集保持有限：NO_ACTION、挂单、撤单、重报价、减仓、清仓、暂停。

动态不确定性集合采用 adapted/causal Wasserstein ball：
$U_t=\{Q:AW_c(Q,\hat P_t)\le\rho_t\}$，
使对手只能使用当时信息，不能通过“未来扰动过去”制造不合法路径。$\rho_t$ 由 session-block bootstrap、锁箱覆盖误差和 regime drift 校准，不作为收益调参旋钮。

有限时域目标：

$\max_\pi\inf_{Q\in U_t}E_Q[\sum_{h=0}^{H-1}\Delta\log W_{t+h}-c_{churn}-c_{inventory}]$

并满足：

- $\sup_Q P_Q(\inf_h W_{t+h}<W_{floor})\le\epsilon_{ruin}$；
- 每条预注册 stress path 的 liquidation buffer、gross/net exposure、单标的仓位和集中度硬约束；
- 股票开盘/资金费/维护截止点的 terminal inventory 约束；
- 数据、规则、订单或账本未知时只允许 risk-reducing action；
- 下单后的最坏可达状态仍在 viability kernel 内。

概率约束只对明确定义的威胁集有效。威胁集外通过低杠杆、隔离/组合限制、fail-closed 和紧急减仓降低损失，但不伪称数学保证。

## 32. 可落地的求解器：分层而非巨型黑箱

每个决策 epoch 采用四层求解：

1. **Belief update**：过滤 $b_t$，更新变点、跳跃、成交与 markout 后验；
2. **Safety shield**：区间传播/压力场景先删除所有不可生存动作；
3. **Robust MPC**：对剩余有限首动作展开短场景树，以 common random numbers 估计 continuation value，并用 causal-DRO 对偶求最坏权重；
4. **Portfolio allocation**：7 标的联合求解风险约束对数增长，再按 lot/min-notional 取整并做可行性修复。

固定场景下，CVaR 用辅助变量写成线性约束；log-growth 为凹目标，连续仓位子问题可用内点/一阶原始-对偶法。离散报价档、TTL 和订单状态用小规模枚举/branch-and-bound，而不是训练不可解释策略。

求解必须返回 primal value、dual bound、optimality gap、约束 slack、迭代数和耗时。超过实时预算、数值失败或 gap 超阈值时，不沿用过期激进动作：只允许 NO_ACTION、撤单或风险减仓。

## 33. 动态资金的正确数学位置

M6 的动态资金是启发式目标分配；M7 将其改为联合风险预算变量 $n=(n_1,\ldots,n_7)$。在情景收益矩阵 $R$ 下求：

$\max_n\inf_{p\in P_t}\sum_s p_s\log(W+R_s^\top n)-\eta\lVert n-n_{prev}\rVert_1$

约束包含 worst-case CVaR、margin buffer、gross/net、单标的 hard cap、HHI/因子暴露和 terminal inventory。$L_1$ 调整成本、最小经济变化阈值、滞回和冷却共同抑制 M6 的分钟级再平衡抖动。

目标资金下降到现有仓位以下时，差额必须转化为有截止时间的 reduce-only liquidation schedule；不能像 M6 一样只阻止新增仓。取整后再次计算全部约束，任何超限都沿风险降低方向修复。

Risk-constrained Kelly 只决定可行域内的增长倾向；实际使用 fractional exposure，并由 ruin bound/CVaR/压力约束取最小仓位，不直接使用全 Kelly。

## 34. 不确定性校准与防过拟合

同时维护三类不确定性：

- aleatoric：跳跃、成交、价格和资金费本身的随机性；
- epistemic：参数、状态、稀疏样本与模型结构不确定性；
- operational：延迟、断线、拒单、对账和写盘风险。

后验预测区间再由按 session/regime 分层的序贯 conformal 校准修正覆盖率；有效校准样本不足时扩大区间。Conformal 只做风险校准器，不把非平稳金融序列误说成无条件 distribution-free 保证。

模型集合至少含 OU、jump-OU、semi-Markov jump-OU、经验 block bootstrap。复杂模型只有在锁箱 predictive log score、PIT/coverage、first-passage Brier、tail loss 和策略配对增量上支配简单模型，才获得权重。

所有阈值、Wasserstein 半径、风险水平、时域和压力集在锁箱前冻结。选择稳定平台，不选择单点最优；M7 内部组件用命名 feature toggle 消融，但正式版本号仍为整数。

## 35. Round 4 实施顺序与验收

实施顺序不变但进一步收紧：

1. 先完成 Round 3 L1–L3 的日志、账本、口径和可复现基础；
2. 再实现 S2 factual queue interval 与 order lifecycle；
3. 校准并认证 S2 generative stress ensemble；
4. 离线实现状态过滤、PIDE/首达概率和参数锁箱；
5. 实现 safety shield、robust MPC 与组合优化；
6. 通过 deterministic replay、故障注入和求解器差分测试；
7. 最后启动 M1–M7、7 标的、F/R 控制的完整并行实验。

新增验收条件：

- 固定锚点 hash 在整个 closed session 不变；
- 过滤器在线输出与仅使用前缀数据的离线重放逐点一致；
- PIDE 网格值与高精度 Monte Carlo 在预设误差内一致；
- DRO 半径增大时风险仓位不得反常增大；
- safety shield 对所有预注册 stress path 无约束穿透；
- 连续解取整后仍满足保证金、notional、lot 与集中度；
- solver timeout/failure 只能降低风险；
- pessimistic/base/optimistic 队列路径均可复放且不共享未来信息；
- M7 的增益不能只来自 simulator-generated data，必须在 factual replay/paper 的公共机会集成立。

## 36. Round 4 研究依据与采用范围

- [Causal/Adapted Wasserstein DRO 的动态对偶](https://arxiv.org/abs/2401.16556)：用于时间一致的动态模型不确定性，不直接作为收益证明。
- [Queue-Reactive order book](https://arxiv.org/abs/1312.0563)：用于状态依赖队列事件与模拟器基线。
- [Deep Queue-Reactive simulator](https://arxiv.org/abs/2501.08822)：仅作为 S2 challenger，不进入策略热路径。
- [Optimal execution under incomplete information](https://arxiv.org/abs/2411.04616)：支持部分信息与 Hawkes 订单流下的执行建模。
- [Bayesian Online Changepoint Detection](https://arxiv.org/abs/0710.3742)：用于因果 run-length/变点后验。
- [带交易成本和止损的均值回归最优停止](https://arxiv.org/abs/1411.5062)：用于入场/退出边界的理论基准。
- [Risk-Constrained Kelly](https://stanford.edu/~boyd/papers/kelly.html)：用于增长与回撤概率约束的凸近似基准。
- [Binance USDⓈ-M ADL risk](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/rest-api/market-data)：ADL 风险必须进入账户压力状态。
- [Binance Futures order/account updates](https://developers.binance.com/en/docs/products/derivatives-trading-portfolio-margin/user-data-streams)：用于订单状态、清算/ADL 与账户事件真值契约。

## 37. Round 5：实盘一致性的严格定义

S2 不追求“生成一条看起来像实盘的路径”，而追求四种一致性：

1. **Protocol equivalence**：同一输入下，过滤器、订单状态机、费用、资金费、保证金和错误处理与交易所公开契约一致；
2. **Observation equivalence**：模拟器只使用实盘可在同一时刻获得的数据，并复制聚合、删失、延迟、乱序和缺失；
3. **Distributional equivalence**：在相同 context 下，成交、markout、延迟、滑点和风险事件分布与实盘误差有界；
4. **Decision equivalence**：同一策略在 paper/shadow/微额实盘中的动作、风险判断和策略排序稳定。

只要公开数据不能识别撮合内部状态，就不能宣称 exact。真值层级固定为：

exchange/account fact > raw received packet > reconstructed state > set-valued hidden state > generative stress。

低层模型不得覆盖高层事实。S2 输出必须带 truth_level、不确定区间和校准版本。

历史路径无法响应本策略的反事实订单。Factual Replay 同时输出 no-impact 主路径和 conservative reactive-impact overlay；策略必须在两者及生成压力族中都成立。以 1 万人民币规模可假设影响较小，但不能假设严格为零。

## 38. S2 单一离散事件内核与多时钟模型

所有 Paper/Replay/Testnet/Live adapter 共享同一事件调度器和状态转移函数。每个事件保存：

- exchange event time、transaction time、output time；
- local wall-clock receive/send time；
- local monotonic receive/decision/send/ack time；
- server-clock offset 区间与测量误差；
- stream/connection/sequence/attempt 标识。

不同 WebSocket、REST 和账户流只形成因果偏序，禁止按本机到达时间伪造唯一“交易所总顺序”。决策快照只能读取其 happens-before closure。

延迟向量联合建模：
$L=(L_{md},L_{decision},L_{out},L_{engine},L_{ack},L_{cancel},L_{reconcile})$。

不独立采样各段延迟。按连接、时段、负载和故障状态拟合经验条件分布；中心体用 empirical copula，尾部用 peaks-over-threshold/极值压力包。重连风暴、DNS/代理切换、磁盘反压和 CPU 抖动进入共同 operational regime。

recvWindow 在撮合到达时按 server clock 校验；本地超时不等于订单失败。503/timeout/unknown response 必须进入 UNKNOWN_PENDING_RECONCILE，先查 user stream/order query，禁止直接生成重复订单。

## 39. 行情观测模型：复制交易所给我们的世界

本地订单簿严格执行 Binance 的 snapshot + diff contract：

1. 先缓冲 diff；
2. 获取 snapshot；
3. 丢弃过旧更新；
4. 首事件满足 $U\le lastUpdateId\le u$；
5. 后续必须 $pu=previous_u$；
6. 档位数量是绝对量，0 表示删除；
7. gap 立即冻结并重新同步。

每次 snapshot/resync 记录原始包 hash、覆盖区间、缓冲事件数、丢弃原因和 book generation。策略不得跨 generation 使用特征或队列状态。

公开 depth 有限且不包含 RPI 隐藏订单；aggTrade 是聚合观测；强平流也可能是时间窗快照。因此观测方程写成：

$Y_t=\mathcal C_t(X_t)+\varepsilon_t$，

其中 $\mathcal C_t$ 显式表示深度截断、事件聚合、隐藏流动性和流删失。模拟器不得把 $Y_t$ 当完整撮合状态 $X_t$。

信号价格、交易价格、风险价格继续分离：

- Index residual：相对固定收盘锚点的经济偏离；
- Contract bid/ask/mid：下单和真实成交；
- Mark：未实现盈亏、保证金和清算；
- FX：人民币初始资金与报告换算，不能混入 USDT 交易 PnL。

A/HK 交易日历、临时休市、半日市、交易所 tradingSchedule、距开盘时间和锚点来源都作为版本化输入。日历未知或锚点 hash 改变时只能降低风险。

## 40. 撮合与订单生命周期数字孪生

订单状态机至少覆盖：

LOCAL_CREATED → SENT → ARRIVED_UNKNOWN/REJECTED/NEW → PARTIALLY_FILLED → FILLED/CANCELED/EXPIRED/EXPIRED_IN_MATCH/CALCULATED。

Modify/Cancel 是独立命令和竞态，不得原地改对象掩盖中间状态。ORDER_TRADE_UPDATE 中 NEW、TRADE、AMENDMENT、CANCELED、EXPIRED 和 CALCULATED 分别落事件；expiry reason、STP、强平、ADL、下市和 reduce-only 冲突不可合并为普通取消。

每次命令使用稳定 intentId、唯一 clientOrderId、attemptId 和可关联 modifyId。UNKNOWN 后重试必须证明前一次未执行；否则只能 query/reconcile。REST response、WebSocket order update 和账户变化三方不一致时按字段权威源处理：订单状态以 order update/query 为准，成交以 trade/fill 为准，余额仓位以 account update/query 为准；任何冲突都产生 reconciliation incident，禁止一种流覆盖另一种流不负责的字段。

Post-only/GTX 在模拟的 exchange-arrival book 上判定，不在 decision-time book 判定。撮合、ACK、user-stream 到达允许不同顺序。部分成交后撤单、撤单 ACK 前成交、重复 update 和迟到 update 都必须可复放。

## 41. 队列部分识别与成交边界

公开 price-level depth 没有订单 ID，且存在隐藏 RPI 流动性，因此自己的绝对排队位置不可观测。对每笔 maker order 维护：

$Q^{ahead}_t\in[Q^-_t,Q^+_t],\quad
P(fill\le h)\in[p^-_h,p^+_h]$。

同价位成交量确定减少队前量；同价位净撤单按“撤在我前方的比例”$\eta_t\in[\eta^-,\eta^+]$传播上下界；新增量默认在已挂订单之后，除非交易所语义或实盘证据说明其他优先级。

价格修改、减量修改、cancel-replace 是否保留优先级必须按 USDⓈ-M 当前接口逐项校准，不能套用 Spot 的 amend-keep-priority 规则。文档没有明确保证时按丢失优先级处理。

每笔成交同时产出 strict/pessimistic、posterior/base、optimistic 三条账本。主结论要求 strict 路径通过；base 用于效率估计；optimistic 只显示识别宽度，不能用于升级。

成交模型采用 competing risks：

$\lambda=(\lambda_{fill},\lambda_{adverseMove},\lambda_{cancelAck},\lambda_{expire},\lambda_{disconnect})$。

fill probability、time-to-fill 和 fill 后 markout 联合估计。校准目标不是提高成交率，而是正确预测“成交且随后不利移动”的联合概率。

对于市价/紧急减仓，按到达时可见深度逐档 sweep，并加隐藏流动性/延迟后的 depletion envelope。超出可见深度部分使用保守 impact curve，不允许按最后一档无限成交。

## 42. 账户、资金与交易所风险真值

统一账户状态必须包含 wallet balance、available balance、cross/isolated、position side、entry/break-even/mark、initial/maintenance margin、leverage bracket、open-order margin、fee tier、BNB fee setting、regular/special funding、insurance/ADL risk。

逐事件不变量：

$Equity=WalletBalance+UnrealizedPnL$，

$\Delta Wallet=RealizedPnL-Commission+Funding+Transfer+Adjustment$。

强平价不能用常数杠杆近似，必须根据当前 bracket、maintenance margin、未成交订单和账户模式重算。跨标的共享保证金时，对一个标的的冲击必须传播到全部仓位。

资金费按实际结算资格、position notional、rateType 和账户事件入账；Regular 与股票分红产生的 Special 永久分列。预测 funding 只用于决策，实际 ledger 只能用交易所结算事实。

限流是状态而非异常字符串：REQUEST_WEIGHT、ORDERS、连接握手、ping/pong、429、-1008 分别建模。风险减仓享受的交易所优先/豁免只按官方语义触发，普通下单不能冒充 reduce-only。

## 43. S2 校准：数字孪生误差模型

对每类可观测结果定义：

$Y^{live}=Y^{sim}_\theta+\delta_\phi(context,action)+\epsilon$。

$\delta_\phi$ 是 simulator-to-live discrepancy，不强迫其为零。按 symbol、session phase、side、queue bucket、volatility、latency regime 分层，用层级模型收缩；数据不足时区间自动变宽。

校准数据依次来自 factual replay、只读 shadow、Testnet 协议验证和未来经授权的极小额度探针。Testnet 只能验证协议和状态机，不能代替主网流动性/成交校准。任何真实下单阶段都必须单独授权，本轮不启动。

S2 不是单模型，而是模型集合：
$\mathfrak S=\{S_{strict},S_{posterior},S_{stress,1},\ldots,S_{stress,K}\}$。

策略升级要求在整个可信集合上的风险约束成立。某个生成器即使边际分布相似，只要产生不真实的条件响应、策略影响或策略排名，就从可信集合移除。

生成模型校准与策略评测严格数据隔离；禁止用“让 M7 收益更高”作为模拟器损失。模拟器只拟合实盘事实和预注册真实性统计。

## 44. M7 联合状态空间与不确定性分解

固定锚点 $A_i$ 不变。每个标的的经济、执行和风险状态分别为：

$x_i=\log(Index_i/A_i)$，
$b_i=\log(Mid_i/Index_i)$，
$m_i=\log(Mark_i/Index_i)$。

$x_i$ 使用 semi-Markov jump-OU；$b_i,m_i$ 使用状态依赖短记忆过程并允许流动性冲击时跳变。这样收敛收益、contract-index basis 和清算 basis 不再混为同一均值回归。

组合共同因子：
$dx=B_s f_tdt+D_s dW_t+dJ_t+\varepsilon_t$。

$B_s$ 采用分层稀疏先验，避免 7 标的小样本协方差爆炸。共同跳跃、A/HK 市场类别、闭盘阶段和临近开盘风险显式进入状态。

不确定性集合定义为：
$\mathcal U_t=\{Q:Q_m\in\mathcal U^{market}_t,\ Q_e\in\mathcal U^{execution}_t,\ Q_o\in\mathcal U^{operation}_t,\ Q\in\Gamma^{causal}_t\}$，
其中 $\Gamma^{causal}_t$ 保留时间和市场—执行—系统故障的联合依赖。不能分别取三个温和边界后再假设它们独立或同时温和。

每个候选动作同时输出 expected、posterior lower bound、DRO worst case 和 named stress loss。任何一个硬生存约束失败即删除候选，不允许收益打分补偿。

## 45. Safety Shield：鲁棒可生存域

定义安全余量：

$h_{margin}=Equity^{stress}-MaintenanceMargin-Reserve$，

以及 position、gross、net、concentration、data age、order uncertainty、time-to-open 等 $h_j(z)$。

动作 $a$ 仅在以下条件成立时可进入优化器：

$\inf_{w\in\mathcal W_t}h_j(F(z,a,w))\ge(1-\gamma_j)h_j(z),\quad\forall j$，

并且从所有后继状态仍存在一条可执行的 emergency reduce/flatten 路径。该离散 barrier/viability 条件保证新增风险不会把系统推进“已经来不及退出”的状态。

低维单标的边界用 Hamilton-Jacobi reachability 或稠密动态规划离线核验；7 标的在线采用可分解保守 barrier、stress polytope 和 margin oracle。高维近似必须偏保守，不能用未验证神经 barrier。

压力集至少含：锚点暂时失效、单边跳跃、跨标的共同跳跃、mark-index 脱钩、盘口骤空、取消失败、ACK 丢失、延迟尾部、funding/special funding、临近开盘、规则变化、API throttle 和进程恢复。

绝对无限跳跃无法保证存活。安全证书必须显示 threat-set version、最坏路径、最小余量和证书覆盖范围。

## 46. M7 词典序鲁棒控制问题

第一层求最大可生存规模：
$q^{safe}=\sup\{q:a(q)\in\mathcal A_{viable}(z_t)\}$。

第二层只在 $[0,q^{safe}]$ 内求：

$\max_{\pi}\inf_{Q\in\mathcal U_t}
E_Q[\sum_h\Delta\log W_h-c_{exec}-c_{churn}]
-\lambda CVaR^Q_\alpha(L)$。

第三层用 tie-breaker 依次最小化 max drawdown proxy、换手、模型敏感度和策略复杂度。这样“好市场强收益”只能来自安全域扩大和更高净机会质量，不能来自放松生存约束。

候选动作价值必须包括：

- 首达目标前先触及 stop/broken/open 的概率；
- maker fill 与 adverse markout 的联合分布；
- cancel/replace 丢失队列的机会成本；
- funding、special funding、fee tier 与最终清仓；
- 当前持仓和所有未决订单的联合最坏暴露；
- NO_ACTION 的机会成本与保留未来选择权的价值。

M7 采用 belief-space receding horizon。远期价值用保守 terminal value approximation；离开训练/校准支持集时 terminal value 归零或变负，禁止乐观外推。

## 47. 求解器与最优性证书

在线求解流程：

1. 更新半马尔可夫/BOCPD belief 和模型权重；
2. Safety Shield 枚举删除不可行首动作；
3. 对剩余动作使用共同随机数生成条件场景树；
4. 用 causal-Wasserstein 对偶或 column-and-constraint generation 求最坏分布；
5. 7 标的仓位子问题用 convex CVaR/log-growth program；
6. tick/lot/min-notional 离散化后做可行性修复；
7. 返回最优首动作与完整证书。

每次求解记录 lower bound、upper bound、duality/optimality gap、约束 slack、最坏场景、活跃约束、迭代数、耗时和 fallback reason。

实时预算到期时采用 incumbent feasible action，不采用未证可行的高价值动作。无可行 incumbent 时只允许 CANCEL、REDUCE_ONLY、FLATTEN、HALT。

离线用高精度动态规划/PIDE、nested Monte Carlo 和独立求解器交叉验证。在线近似相对离线 oracle 的 regret 必须分 context 报告；oracle 使用未来信息时只能称 upper bound，不能作为可交易基线。

## 48. 日志：从记录结果升级为因果事件溯源

日志分为五条不可混写的数据平面：

1. Raw Wire Journal：收到/发出的原始 bytes、headers、endpoint、connection、压缩和解析状态；
2. Canonical Domain Events：规范化 market/order/account/system 事件；
3. Decision Journal：特征、belief、候选、约束、价值和最终动作；
4. Economic Ledger：逐 fill/fee/funding/transfer 的双式记账与 PnL 分解；
5. Metric/Report Store：可重算派生结果，不是真值源。

每条记录统一包含 schemaVersion、runId、ledgerId、strategyId、simulatorId、eventId、parentId、traceId、streamId、seq、所有时钟、entityId、truthLevel、payloadHash、codeVersion、configHash、modelVersion 和 uncertainty。

Raw → Canonical → State → Decision → Intent → Request → ExchangeEvent → Fill → Ledger → Metric 形成可查询 DAG。跨流只记录已知因果边；未知顺序保持并发，不能随意排序。

任一 NO_ACTION 必须保存完整 candidate set、首个失败硬约束、所有 slack、LCB、NO_ACTION value 和模型支持度。任一风险减仓必须区分 target reduction、emergency reduction、pre-open、funding、margin、data failure 和 operator action。

## 49. 日志可靠性与可恢复性

真值与账本 writer 禁止 try-send 丢弃。采用有界 write-ahead queue：

- 到高水位：停止产生新增风险；
- 到临界位：撤单并进入 risk-reducing only；
- fsync/磁盘失败：HALT_NEW_RISK，保留内存紧急事件并报警；
- 恢复后从 checkpoint + hash chain 重放。

处理语义采用 at-least-once ingestion + idempotent state transition，不伪称分布式 exactly-once。每个原始包和交易所事件以稳定键去重，重复本身仍作为 telemetry 记录。

文件采用按大小/时间分段的 append-only journal，临时文件原子 rename，段尾 checksum、前后 hash chain、事件计数和最小/最大时间。checkpoint 保存账本、订单状态、book generation、过滤器 belief、随机数状态和最后消费 offset。

run manifest 额外固定：git commit、dirty patch hash、GNU toolchain、binary/config/schema/data hash、OS/CPU、timezone、dependency lock、seed tree、交易所文档版本、合约规格、费率、账户模式、启动/退出来源和退出码。

敏感凭证永不进入日志；请求只保存安全指纹和业务参数。日志 schema 向后兼容 reader，迁移必须保留原始段，不能覆盖旧实验。

## 50. 决策证书与事故包

每次动作额外生成 DecisionCertificate：

$C_t=(I_t,B_t,\mathcal A_t,\mathcal U_t,\mathcal W_t,
V^-,V^+,slack,gap,a^*,fallback)$。

其中 $I_t$ 是可见信息闭包，$B_t$ 是 belief，$\mathcal A_t$ 是候选集，$\mathcal U_t/\mathcal W_t$ 是模型与压力集合。证书能够回答“为什么交易、为什么这个量、为什么这个价格、为什么当时没退出”。

每次 fill 保存 queue interval、arrival book、maker/taker、commission、mark/index/mid、100ms/1s/5s/30s/terminal markout、对应候选预测和随后 realized outcome。

Incident bundle 自动触发条件包括：大亏、硬约束逼近、unknown order、reconciliation mismatch、writer backpressure、book resync、clock jump、solver failure、异常 funding、强平/ADL、进程非正常退出。

事故包包含事件前后窗口、原始包、DAG 子图、状态快照、订单时间线、账本、solver certificate、线程/资源指标和重放命令。任何事故必须能在隔离环境确定性重演到同一 hash。

## 51. 指标：模拟器真实性认证

真实性不能压缩成一个可被优化欺骗的总分。四张独立证书分别通过：

### 51.1 Protocol Conformance

- snapshot/diff/resync、订单状态转移、重复/乱序幂等；
- filter、tick/step/min-notional、GTX、reduce-only、STP、expiry；
- fee/funding/margin/liquidation/ADL；
- recvWindow、timeout UNKNOWN、429/-1008、listenKey expiry；
- checkpoint/recovery 后状态一致。

协议项要求 100% 通过；不存在“平均 99.9% 就够”。

### 51.2 Observation Fidelity

按 symbol × session phase × regime 比较真实与模拟：

- inter-arrival、event type、size、spread、depth、imbalance；
- return/volatility/vol-of-vol、tail index、jump、duration；
- order-flow autocorrelation、cross-level/cross-side dependence；
- mark-index/contract-index basis、funding、流动性恢复时间；
- gap、重复、乱序、重连和延迟联合分布。

报告 Wasserstein、energy distance、MMD、conditional classifier two-sample AUC、quantile error 和 tail exceedance error。所有特征先用真实训练窗尺度标准化，不能让高方差变量支配距离。

### 51.3 Execution Fidelity

- fill probability：Brier、log loss、reliability curve、ECE；
- time-to-fill/cancel：survival calibration、integrated Brier、censoring-aware concordance；
- queue interval：coverage、平均宽度、conditional coverage；
- fill 后 markout：均值、分位数、expected shortfall、方向/状态条件误差；
- market/urgent order：implementation shortfall、depth sweep、尾部滑点；
- ACK/cancel/reconcile latency：联合分位数、copula/tail dependence；
- post-only reject、partial fill、cancel race、STP、expired reason 频率。

预测区间只有“覆盖率高且宽度足够窄”才合格。用无限宽区间达到覆盖率视为失败。

### 51.4 Decision Fidelity

在相同 raw prefix 上比较 paper/shadow/live：

- feature/belief/action 一致率；
- risk state 与 first-failed-constraint 一致率；
- order intent 到交易所状态的转移混淆矩阵；
- predicted 与 realized PnL component；
- 各方法策略排序的 Kendall/Spearman、top-k stability；
- 模拟器参数扰动下 pairwise delta 符号稳定率。

策略排名不稳定时，模拟器不得用于选择 M7，即使单项 stylized facts 很漂亮。

## 52. 策略与生存指标的最终分层

Primary survival：

- liquidation/ADL/ruin upper confidence bound；
- stress 后最小 margin buffer；
- max drawdown、duration、time-under-water、recovery；
- CVaR/expected shortfall、worst excursion、overshoot area；
- emergency exit completion probability/time/cost。

Primary economic：

- exact net PnL 与 return on allocated capital；
- net log-growth、Calmar、Omega、Ulcer；
- PnL per gross-notional-hour、margin-hour 和 opportunity；
- convergence capture ratio、good-market capture、bad-market avoidance；
- fee/funding/adverse-selection/emergency/rebalance drag。

Primary decision：

- canonical opportunity recall/precision；
- $p_{conv}$、edge、tail loss 与 regime posterior calibration；
- false mean-reversion entry、late exit、missed safe opportunity；
- realized regret 对可交易基线；clairvoyant 仅列 upper bound；
- NO_ACTION 的避免损失和机会成本。

Primary execution：

- maker/taker 比例、有效/已实现 spread；
- queue-adjusted fill、partial ratio、time-to-fill；
- 100ms/1s/5s/30s/terminal markout；
- quote age、cancel-to-fill、reprice churn；
- reduce-only schedule 的完成率、超时率和成本。

Primary portfolio：

- target/actual tracking、hard-cap overshoot area；
- gross/net/factor exposure、HHI、marginal CVaR；
- risk-budget utilization、rebalance turnover；
- shock propagation、共同跳跃损失与分散化失效；
- M6→M7 动态资金的配对增量。

## 53. 日志与运行可靠性指标

- raw/canonical/ledger event completeness；
- unknown parent、broken trace、orphan fill、duplicate/conflict；
- writer queue high-water、backpressure duration、fsync latency；
- checkpoint age、recovery time、replay hash mismatch；
- book generation count、resync duration、stale exposure；
- clock offset/uncertainty、negative latency、causal-order violation；
- reconciliation count、金额、持续时间、unexplained PnL；
- CPU/memory/network/disk、decision/solver p50/p95/p99/max；
- graceful/forced/crash exit 与最后 durable event。

硬指标：账本事件 drop=0、unexplained PnL=0、orphan fill=0、硬约束穿透=0、replay hash mismatch=0。任一非零则该 run 不具备策略比较资格。

## 54. 无限运行下的统计推断

“时间无限”意味着允许持续查看结果，普通固定样本 p-value 会失效。统计单位仍为 closed session、独立 anchor excursion 和预注册 stress seed，不是 tick 或 30 秒 PnL。

同时维护：

1. 固定冻结 horizon 的 block bootstrap/HAC/SPA/DSR；
2. session-block 的 anytime-valid confidence sequence/e-process；
3. M1–M7 同一事件流与 seed 的 paired delta；
4. symbol/session/regime 的层级效应与 leave-one-symbol-out 稳健性。

连续查看只能依据预先定义的 confidence sequence；不能看到好结果就停。正式升级仍需独立锁箱窗口，且置信下界超过最小经济效应。

对每个增量 $M_k-M_{k-1}$ 报告 mean/median/Hodges-Lehmann、paired win rate、block-bootstrap CI、$P(\Delta>0)$、风险非劣概率和贡献集中度。多版本搜索继续用 White Reality Check/Hansen SPA 与 Deflated Sharpe。

## 55. 真实性与策略升级门禁

Gate 0 — Specification：交易所契约、公式、schema、threat set、主指标和阈值冻结。

Gate 1 — Deterministic Conformance：golden packets、property tests、状态机 model checking、故障注入和重放 hash 全通过。

Gate 2 — Historical Factual Replay：全部 7 标的跨 session 的 protocol/observation 指标通过；strict queue 和 impact overlay 下生存。

Gate 3 — Shadow：使用主网只读行情/账户语义，策略产生意图但不发订单；检查 decision、延迟、限流和运行可靠性。

Gate 4 — Testnet：只验证认证、订单生命周期、错误、恢复和对账，不用于证明成交收益。

Gate 5 — Micro Live：只有经用户单独授权才运行；极小额度、硬损失上限、自动 kill、逐订单探针，用于 fill/latency/markout 和 simulator discrepancy 校准。

Gate 6 — Limited Production：M7 必须在独立锁箱、strict simulator family、shadow/micro-live 证据中同时通过，且相对 M6 收益显著、风险非劣。

任何 Gate 失败都回到对应模型层，不允许通过调策略掩盖模拟器失败。实盘证据逐级增加权限，不一次性跳级。

## 56. Round 5 实施切片

P0：完成已有 L1——reduce-only 方向、lifetime 指标、reject taxonomy、manifest/exit/writer 错误。

P1：Raw Wire Journal、多时钟、因果偏序、统一 Envelope、无丢失账本。

P2：Binance conformance——book generation、UNKNOWN reconcile、完整订单/账户/资金费/限流状态机。

P3：S2 factual queue interval、strict/base/optimistic ledger、market-order depth sweep 和 impact overlay。

P4：真实性 metric suite、条件校准、simulator discrepancy 与认证报告。

P5：M7 belief、首达 PIDE、execution competing risks、Safety Shield。

P6：causal-DRO MPC、7 标的组合优化、求解证书与 fail-safe。

P7：完整 M1–M7 × 7 标的 × F/R 公共事件流实验，生成每方法每标的全部报告。

Round 5 仍是设计轮，未修改 engine 代码、未编译、未启动实验。下一实际动作应从 P0 开始，不跨过日志与协议基础直接实现 M7。

## 57. Round 5 新增研究与官方依据

- [Binance：正确维护 USDⓈ-M 本地订单簿](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/websocket-market-streams/How-to-manage-a-local-order-book-correctly)：规定 snapshot/diff、U/u/pu 和 gap 重同步语义。
- [Binance USDⓈ-M Market Data](https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-api/market-data)：公开深度有限且不含 RPI 订单，因此队列必须部分识别。
- [Binance USDⓈ-M General Info](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/general-info)：timeout/503 可能为执行状态未知、recvWindow 与 -1008 风险减仓语义。
- [Binance USDⓈ-M User Data Streams](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/user-data-streams)：订单、AMENDMENT、STP expiry、强平/ADL、账户和 listenKey 事件。
- [Binance USDⓈ-M Change Log](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/change-log)：TradFi tradingSchedule、Special funding 与接口变化必须版本化。
- [Get Real：LOB 模拟器真实性指标](https://arxiv.org/abs/1912.04941)：支持以多类 stylized facts 和真实性差距认证模拟器。
- [LOB-Bench](https://arxiv.org/abs/2502.09172)：支持条件化、多维和策略响应层面的生成式 LOB 评估。
- [Limit Order Book Simulations Review](https://arxiv.org/abs/2402.17359)：用于选择 Queue-Reactive、Hawkes、agent-based 与生成模型的适用边界。
- [Optimal Execution under Incomplete Information](https://arxiv.org/abs/2411.04616)：支持部分信息、marked Hawkes 与冲击下的执行控制。
- [Causal/Adapted Wasserstein DRO](https://arxiv.org/abs/2401.16556)：用于不使用未来信息的动态分布鲁棒优化。
- [Safe Anytime-Valid Inference](https://arxiv.org/abs/2210.01948)：用于无限运行和持续查看结果时的置信序列/e-process。

## 58. Round 6 目标：把核心机制变成可证伪假设

核心假设不是“策略曾经盈利”，而是：在 A/HK 标的闭盘、锚点有效且尚未重新开盘的条件下，Binance TradFi 永续指数相对官方固定收盘锚点的残差，存在可重复、可在剩余闭市时间内兑现、扣除可执行成本后仍有价值的条件均值回归。

本轮只新增所有 M1–Mxx 共用的 Hypothesis Verification Layer，不新增 M8，不改变任何历史版本。结构性市场假设、执行可交易性和具体策略收益必须分开验证。

证据链分为五层：

1. H-data：锚点、映射、FX、公司行动、时间和行情足以定义无歧义残差；
2. H-drift：偏离后条件漂移方向朝向固定锚点；
3. H-center：长期中心在预注册容差内等价于零，而非仅仅“无法拒绝不为零”；
4. H-deadline：在开盘、风险止损或数据失效前，触达目标带的概率和时间具有实用价值；
5. H-economic：按严格成交模拟扣除手续费、滑点、资金费和冲击后，保守净边际仍为正。

任何一层失败都必须如实标记；不得用后一层偶然盈利掩盖前一层失败，也不得由策略筛选后的成交样本反推市场规律。

## 59. 固定锚点、残差与无选择偏差样本

对标的 i、闭市 session s，使用在闭市开始前已确定并冻结、且已映射到合约报价单位的有效官方收盘锚点 A(i,s)。原始股票收盘价固定不变；若合约规格要求币种/比例转换，则转换规则与所用 FX reference 也必须在 session 前冻结。锚点须携带来源、发布时间、币种、FX 版本、合约映射、除权除息/公司行动和内容哈希。任何未知调整不得按 0 处理；迟到修正使该 session 失效，不可静默回写历史。

主结构残差定义为：

$$x_{i,s,t}=\log(I_{i,t}/A_{i,s}),$$

其中 I 是 Binance 指数价格。合约 mid、mark 与可成交 bid/ask 的残差分别记录，只用于基差、清算与经济层，不能污染结构层对指数锚定机制的检验。

建立 Canonical Hypothesis Opportunity 流：在预注册时钟网格或一次独立 anchor excursion 首次穿越残差桶边界时生成样本，不取决于任何 M 版本是否下单。所有方法引用同一个 evidence_id；NO_ACTION 也有完整结果。

主统计单位是 completed closed session 或相互分离的 anchor excursion，不是 tick。预注册 1、5、15、30、60 分钟及 terminal/open-deadline horizon；重叠 horizon 使用 session block/HAC，主确认集优先采用非重叠 excursion。

样本切分永久冻结为 discovery、calibration、lockbox。模型、阈值和残差桶只能在前两者调整；lockbox 只运行一次正式确认。

## 60. 方向收缩、动态形状与锚点中心检验

对每个 horizon h，同时计算：

$$D_h=-\operatorname{sign}(x_t)(x_{t+h}-x_t),\quad C_h=|x_t|-|x_{t+h}|,$$

$$R_h=\log\frac{|x_t|+\epsilon}{|x_{t+h}|+\epsilon},\quad Q_h=1\{|x_{t+h}|<|x_t|\}.$$

D、C、R 为正和 Q 大于预注册基准才表示朝锚点收缩；结果按标的、方向、初始残差桶、闭市阶段、波动/流动性/消息 regime 报告。

主条件局部投影：

$$x_{t+h}-x_t=\alpha_h+\beta_h x_t+\gamma_h^\top Z_t+u_{i,s,t,h},$$

其中 Z 只能含 t 时刻可得信息，并加入 symbol/session/calendar 固定效应。核心方向证据为关键 horizon 上 beta_h 的置信上界小于 0，并满足最小经济效应，而不只是 p<0.05。

再拟合支持不规则时间与跳跃的状态条件模型：

$$x_{t+\Delta}=\mu_z+(x_t-\mu_z)e^{-\kappa_z\Delta}+\varepsilon_{t,\Delta}.$$

必须联合证明 kappa_z 大于预注册最小速度，并对 |mu_z|<epsilon_anchor 做 TOST/等价性检验。未拒绝 mu=0 不构成锚点正确证据；epsilon_anchor 由报价精度、锚点误差和最低可交易边际事先确定。

另估计非参数漂移 g_z(x)=E[Delta x/Delta t | x,Z=z] 及同时置信带；在可交易残差区要求 x*g_z(x)<0，用于暴露非线性、阈值效应和多空不对称。ADF/PP/KPSS 只作辅助诊断，不能单独通过核心假设。

## 61. 开盘截止首达与经济可交易性

定义竞争风险：tau_target 为首次进入锚点目标带，tau_adverse 为首次触发预注册风险带，tau_open 为开盘/强制退出截止，tau_invalid 为锚点或数据失效。

估计条件累计发生概率：

$$p^*(x,z)=P(\tau_{target}<\tau_{adverse}\wedge\tau_{open}\wedge\tau_{invalid}\mid x,z),$$

并报告 restricted mean time-to-target、未到达比例、目标前最大不利偏离和右删失原因。延迟 horizon 尚未成熟时保持 pending/right-censored，绝不能记作“未回归”。

结构层通过后，另建立与任何 M 无关的标准化虚拟交易探针：在相同 opportunity 上按 S2 strict ledger 的真实 bid/ask、队列区间、延迟、手续费、资金费、冲击和强制退出规则执行。

净实现价值为：

$$G_h^{net}=G_h^{anchor}-fee-slippage-funding-impact-risk\ buffer.$$

H-economic 要求 session-block anytime 置信下界高于预注册最小净边际，同时 target-before-adverse 概率下界超过门槛。结构有效但净边际不正时结论必须是“统计回归、当前不可交易”，不得宣称策略有效。

策略层仍单独比较 M1–Mxx 的真实账本收益、风险和机会成本；它回答“谁利用得更好”，不负责证明“现象存在”。

## 62. 反证、安慰剂与替代解释排除

每个正式样本同步计算、但不用于交易的安慰剂锚点：同标的同日历 regime 的日期置换收盘、尺度匹配的跨标的收盘、与残差分布匹配的合成锚点、以及预注册的 close 前时间点价格。使用未来收盘的安慰剂仅可离线标记，严禁进入特征或决策。

真实官方收盘锚点必须在配对 session 统计上显著优于安慰剂，而不只是自身 beta<0。否则可能只是一般价格负自相关、波动衰减或构造性机械回归。

必须额外报告：

- 开市期间相同时间长度的负对照；
- 距官方收盘远近与收缩强度的剂量响应；
- 控制 BTC/美股指数、FX、整体 TradFi 因子后的残差漂移；
- 重大消息、跳跃、流动性坍塌和开盘邻近的交互项；
- leave-one-date-out、leave-one-symbol-out、单边方向剔除和极端日剔除。

若证据只存在于单一标的、单一方向、少数日期或某个残差桶，结论降级为 conditional support，并只允许策略在证据支持的 cell 中增加风险；不可外推为全市场规律。

## 63. 无限运行、多重检验与防止 p-hacking

持续实验使用 completed-session block 的 anytime-valid confidence sequence/e-process；普通固定样本 p-value 只在冻结 horizon 报告。任何人在任意时刻查看结果都不得改变错误率。

primary family 预注册为少量关键 horizon、7 个正式标的和两方向，采用层级 closed testing/Holm 控制 FWER；探索性 regime/残差桶使用 BH 或 e-value FDR，并明确标注 exploratory。

版本、阈值和模型搜索不共享 lockbox。模型比较采用 prequential 一步向前评分；正式结论需在后续未见 session 上复制。所有停止、排除、锚点修正和缺失规则写入 manifest。

## 64. 假设日志、逐股指标与证据面板

新增与 Method ledger 解耦的 HypothesisOpportunity：evidence_id、symbol/session、anchor 全谱系、t0、x0、方向/桶/regime、可观测控制量、数据有效性、各 horizon due time。

新增 HypothesisOutcome：x_h、D/C/R/Q、target/adverse/open/invalid 时间与顺序、删失原因、最大有利/不利偏离、strict/base 执行结果、完整成本分解、暴露中的数据 gap。记录生成后不可覆盖，只能追加 correction/invalidation event。

每个标的和组合层至少输出：

- horizon contraction curve 与 simultaneous confidence band；
- beta_h 局部投影曲线、非参数 drift shape 和多空不对称；
- kappa、half-life、mu 及 anchor-equivalence 区间；
- target/adverse/open competing-risk CIF 与剩余闭市时间分层；
- 官方锚点相对全部 placebo 的配对优势和 rank；
- gross-to-net edge waterfall、严格队列上下界与成本敏感度；
- symbol × direction × residual bucket × session phase × regime 证据热图；
- anytime evidence trajectory、有效样本量、删失率和 leave-one-out 稳健性。

汇总报告必须同时给 micro average、symbol-equal macro average、最弱标的、最坏 regime 和贡献集中度，防止大样本标的掩盖失败标的。每个 M1–Mxx 继续获得逐方法逐标的完整指标，但核心假设面板只计算一次并由全部方法共享。

## 65. 预注册判决与策略使用规则

每个 symbol × direction × residual bucket × session phase × regime cell 输出四态判决：SUPPORTED、CONDITIONALLY_SUPPORTED、INCONCLUSIVE、REJECTED。

SUPPORTED 至少要求同时满足：数据层通过；关键 horizon 的 beta_h 和 D/C 下界满足最小效应；mu 通过锚点等价性；target-before-adverse/open 下界达标；真实锚点显著优于 placebo；strict 净边际下界为正；证据不由单日支配。

阈值不得从同一测试集反复调到通过。epsilon_anchor、最小收缩、最低首达概率和最低净边际由锚点误差、总摩擦、风险预算和开盘前剩余时间推导并在 manifest 冻结。

策略可将判决和实时置信下界作为 eligibility/risk multiplier：SUPPORTED 正常使用，CONDITIONALLY_SUPPORTED 降额且限定 cell，INCONCLUSIVE 只记录不增险，REJECTED 禁止该机制开仓。Safety Shield 始终具有更高优先级。

该门控属于可变策略层；“官方收盘是候选固定锚点、必须接受独立证伪”属于不可变研究原则。若核心假设整体被拒绝，应停止新增版本优化，而不是继续拟合收益。

## 66. Round 6 实施切片

H0：冻结 hypothesis manifest、样本单位、horizon、最小效应、placebo 和多重检验 family。
H1：实现独立 opportunity/outcome 事件、锚点谱系与 right-censor completion worker。
H2：实现局部投影、TOST、跳跃 OU、非参数漂移与 competing-risk 分析。
H3：接入 S2 strict 标准化交易探针和 gross-to-net 经济层。
H4：生成逐股证据面板、anytime evidence、placebo 与 leave-one-out 报告。
H5：完成 discovery/calibration 后冻结；只在新 lockbox session 作正式判决，再供 M1–Mxx eligibility 使用。

Round 6 仍是设计轮，未修改 engine 代码、未编译、未启动实验。实际实施仍须先完成 Round 5 的 P0–P3 数据、日志和 strict simulator 基础，再并行推进 H0–H4。

## 67. Round 6 方法依据

- Dickey–Fuller 单位根检验用于辅助识别持久性，但不等价于证明固定锚点正确。
- KPSS 以平稳性为原假设，可与单位根检验互补；结构突变、跳跃与短 session 下仍仅作诊断。
- TOST/等价性检验用于主动证明长期中心位于预注册锚点容差内，避免把“不显著”误读为“相等”。
- Jordà local projections 用于直接估计不同 horizon 的状态条件收缩路径，减少错误指定单一动态模型的依赖。
- competing risks/survival 处理 target、adverse、open deadline 与右删失，回答回归是否及时。
- Safe anytime-valid inference/confidence sequences 用于无限运行和持续查看，session block 保留序列依赖。

主要文献：Dickey & Fuller (1979), Distribution of the Estimators for Autoregressive Time Series With a Unit Root；Kwiatkowski et al. (1992), Testing the Null Hypothesis of Stationarity；Jordà (2005), Estimation and Inference of Impulse Responses by Local Projections；Lakens (2017), Equivalence Tests；Safe Anytime-Valid Inference (2023)。

## 68. Round 7 核心结论：从高拟合升级为可识别、可校准、可证书化

Round 7 不创建 M8。S2、M7 和 Round 6 假设层的方向保留，但补齐三个缺口：

1. observed residual 的收缩不等于固定锚点机制，必须排除共同因子、机械负相关、测量误差和一般波动衰减；
2. 单一“最像历史”的模拟器不能代表实盘，必须给模拟器误差建立有限样本可信集合并让策略在集合内生存；
3. 静态 CVaR、平均 PnL 和普通 Monte Carlo 不能可靠约束动态回撤与罕见破产，必须采用时间一致风险递归和稀有事件估计。

因此“最优解”严格定义为：在冻结信息集、动作网格、可信模型集合、风险容忍度和计算时限内，通过安全可行性、取得可验证 primal/dual gap 的最优策略。未知威胁集之外不承诺全局最优或绝对存活。

不可变：官方收盘候选锚点固定、先证伪后使用、硬安全优先、M1–Mxx 公共事件流并行、模拟器与策略隔离。可变：模型权重、置信半径、风险预算、动作规模、执行方式和 evidence cell eligibility。

## 69. 锚点机制的结构分解与可识别目标

对每个标的定义观测残差 $x_t=\log(I_t/A)$，再将其分解为：

$$x_t=u_t+\ell_t^\top f_t+q_t+\nu_t,$$

其中 $u_t$ 是待验证的 anchor-specific mispricing，$f_t$ 是决策时可见的全球/类别/FX 因子，$q_t$ 是指数构造、合约映射或时段性偏差，$\nu_t$ 是观测误差。策略不能假定 $x_t=u_t$。

结构目标不是声称“闭市随机导致回归”，而是识别条件收缩函数：

$$g_h(u,z)=E[u_{t+h}-u_t\mid u_t=u,Z_t=z,valid\ anchor,closed],$$

并检验可交易区域内 $u\,g_h(u,z)<0$、长期中心等价于 0、且效应强于负对照。若顺序可忽略性、平行趋势或有效工具变量等识别假设不成立，只能报告稳健条件关联，禁止使用 causal 字样。

$f_t$ 至少覆盖 TradFi 横截面共同成分、BTC/加密风险、相关美股/指数期货、USD/CNY 或 USD/HKD reference、波动与流动性；任何因子都必须满足当时可得和 source-time 审计。

## 70. 正交化检验与机械均值回归防护

对 $Y_h=x_{t+h}-x_t$、$X=x_t$ 和预处理控制量 $Z_t$，在 session-block 外样本拟合 $m_h(Z)=E[Y_h|Z]$ 与 $e(Z)=E[X|Z]$，构造：

$$\tilde Y_h=Y_h-\hat m_h(Z),\quad \tilde X=X-\hat e(Z),$$

$$\hat\theta_h=\frac{\sum \tilde X\tilde Y_h}{\sum \tilde X^2}.$$

该 Neyman-orthogonal score 降低共同因子拟合误差对收缩系数的一级敏感性。主实现用可解释 ridge/GAM/稀疏线性模型；时间序列 DML 只作离线 challenger，并采用带 purge/embargo 的 forward block cross-fitting，不能随机打散 tick。

由于 $Y_h$ 含 $-x_t$，同一时点的测量噪声会机械制造负系数。必须同时执行：

- 使用 Binance index 作为主结构价格，mid/mark 只作平行稳健性检验；
- 用 pre-window 多快照稳健均值定义 $X$，future window 独立定义结果；
- 以足够滞后的残差或独立参考源作 IV/errors-in-variables sensitivity；
- 排除最短 horizon 后复核，报告噪声方差从 0 到上界的 SIMEX/状态空间敏感度；
- 对 stale/step/index-component update 单独分层，不把一次指数刷新当经济回归。

若去除机械噪声后效应消失，H-drift 判为 REJECTED，而不是调大交易阈值。

## 71. 闭市特异性与反事实证据

在官方收盘和下一次开盘两侧建立预注册 event-time panel，估计：

$$\Delta_h x_t=\alpha_{i,s,h}+\beta_h x_t+\delta_h(x_t\times Closed_t)+\Gamma_h Z_t+\epsilon_{t,h}.$$

$\delta_h<0$ 表示闭市相对可比开市窗口的额外收缩，但只有在无提前反应、可比窗口和平行趋势诊断通过时才具有准因果解释。否则保留为机制特异性证据。

增加三重对照：同一标的开市窗口、同一时刻但非 A/HK 锚点的 TradFi 合约、同一残差幅度的日期置换锚点。官方固定锚点必须在收缩速度、零中心等价和 deadline first-passage 三项均胜出。

实施 partial-identification sensitivity：给未观测共同冲击与 $X,Y$ 的相关强度设置区间，输出使 $\theta_h$ 翻号所需的最小 confounding strength。结论必须显示 point estimate、orthogonal estimate、IV/EIV 区间和最坏敏感度边界。

这层验证仍独立于策略行为；M1–Mxx 的下单、成交和持仓不能进入结构样本筛选。

## 72. S2 有限样本可信集合，而非单点校准

分离 aleatoric path risk、parameter uncertainty、model-form discrepancy 和 observation censoring。对 simulator family $S_k$ 定义下一 session 预测统计向量 $T$，并用 calibration-only 数据形成：

$$C_{1-\delta}=\{(k,\theta):d_w(T^{real},T^{sim}_{k,\theta})\le r_{1-\delta}(z)\}.$$

$d_w$ 同时包含路径、条件响应和尾部距离；$r_{1-\delta}(z)$ 由按 session 的 block bootstrap 与 prequential conformal residual 校准，不允许根据 M7 收益调节。

动态可信分布集合：

$$\mathcal U_t=\bigcup_{(k,\theta)\in C_{1-\delta}}\{Q:AW_c(Q,P_{k,\theta})\le\rho_t(k,\theta,z_t)\},$$

其中 adapted Wasserstein 保留信息流；模型索引、参数和 discrepancy 必须联合变化。分别挑选最温和的市场、执行和系统模型后再相乘属于非法乐观组合。

Conformal 层只校准 loss/quantile 的经验覆盖，不宣称在任意金融非平稳序列中分布无关。报告 rolling coverage、stress-cell coverage、coverage gap、interval width 和 effective sample size；ESS 过低或近期连续超界时扩大半径直至 HALT。

2025–2026 的 anytime/conformal 方法仅作 challenger；通过合成真值、历史锁箱和 fault injection 后才能成为安全边界来源。

## 73. 极端市场与破产概率的稀有事件求解

普通 Monte Carlo 中“跑了 N 条路径、一次没爆仓”不能证明安全。若独立路径零失败，单侧置信度 $1-\alpha$ 下的失败概率上界仍为：

$$p_{fail}^{upper}=1-\alpha^{1/N}\approx-\log(\alpha)/N,$$

即 95% 单侧置信上界约为 $3/N$。所有报告必须显示该上界，禁止输出 ruin probability=0。

S2 stress ensemble 对共同跳跃、流动性抽空、mark-index 脱钩、取消失败和延迟风暴使用 multivariate POT/regular-variation tail；bulk 与 tail 在冻结阈值处连续拼接，尾部依赖用极端相关或 tail copula 校准。

对 $\tau_{ruin}$、强平、无法在开盘前退出等罕见事件使用 adaptive multilevel splitting：按 margin slack、drawdown 和 unresolved-order exposure 设置中间 level，复制接近危险边界的路径，使每层存活样本量近似稳定。

再用独立 importance-sampling/命名 stress seeds 交叉验证。每项输出估计值、置信区间、有效样本量、level sensitivity、最可能失败路径和 simulator family 贡献；两种估计不一致时取更坏上界并标记未认证。

## 74. M7 时间一致的词典序鲁棒控制

扩展状态为 $z_t=(belief,orders,inventory,cash,margin,H_t,W_t,time\ to\ open,health)$，其中 $H_t=\max_{u\le t}W_u$，drawdown state 为 $D_t=1-W_t/H_t$。

第一层 Safety Shield 求鲁棒可生存动作集：

$$\mathcal A_{safe}(z_t)=\{a:\sup_{Q\in\mathcal U_t}P_Q(\tau_{liq}\wedge\tau_{ruin}\le T|z_t,a)\le\epsilon_{ruin},\ h_j^{worst}\ge0\ \forall j\}.$$

第二层在安全域内最大化动态鲁棒增长：

$$V_t(z)=\max_{a\in\mathcal A_{safe}}\inf_{Q\in\mathcal U_t}\left\{E_Q[\Delta\log W_{t+1}-c_t]+\mathfrak R_t^Q[V_{t+1}]\right\},$$

$\mathfrak R_t$ 使用嵌套条件风险映射，而不是每天重算一个静态 CVaR；否则昨天认为可接受的计划可能在今天产生时间不一致反转。

第三层对近似同值动作依次最小化 drawdown upper bound、尾部模型敏感度、换手、未决订单复杂度；鲁棒价值相对 NO_ACTION 未超过统计误差与 solver gap 时必须 abstain。

Risk-constrained Kelly 只提供安全规模上界，不直接决定仓位。最终规模还需同时满足保证金、可退出深度、队列/延迟、组合共同跳跃和开盘 deadline 约束。

## 75. 一万元动态资金与“好市场强收益”机制

总资本仍为 10,000 CNY 报告口径；交易账本以实际 USDT 余额为真值，CNY 仅按有时间戳的 FX 转换展示，不混入自融资约束。

每个候选 cell 的风险乘子定义为：

$$r_{cell}=r_{evidence}\,r_{regime}\,r_{liquidity}\,r_{deadline}\,r_{drawdown},\quad 0\le r_{cell}\le1,$$

其中 evidence 来自 Round 6 独立假设层，其他项来自实时可观测风险。任一 REJECTED/Broken/stale 项使乘子为 0。

组合规模：

$$q^*=\min(q_{viability},q_{margin},q_{exit},q_{drawdown},q_{robustKelly})\times r_{cell}.$$

“大好市场强收益”不靠放宽硬约束，而来自：更多 SUPPORTED cell、更窄模型集合、更高 target-before-adverse 概率、更低成本和更大的可退出安全容量。证据增强可平滑扩容；单次盈利、短窗 Sharpe 或 posterior 均值跳升不能突然加杠杆。

引入 capital recovery hysteresis：回撤后降额快、恢复慢；只有累计 evidence CS、风险覆盖和账户对账连续达标才逐级恢复，防止震荡状态频繁放大/缩小仓位。

## 76. 可计算求解器与最优性证书

离线层：对 symbol × regime × time-to-open 网格求 jump-OU first-passage PIDE、viability kernel 和 terminal value；使用 monotone finite-volume/semi-Lagrangian scheme，并以网格加密、边界扩张和 Monte Carlo 首达交叉验证误差。

在线层采用两阶段：先枚举有限首动作并由 Safety Shield 剪枝；再对 7 标的规模做 convex/conic master allocation。最坏分布用 causal-DRO dual/cutting-plane oracle，发现新 adversarial path 后加入约束直至上下界收敛。

每次决策输出：

- primal feasible value 与 dual/worst-case upper bound；
- absolute/relative optimality gap 和 discretization/model-error bound；
- binding risk constraints、shadow prices 和最坏 adversarial scenario；
- solve time、iteration、cache/grid version 与 deterministic replay hash；
- timeout/failure 时采用的已验证 incumbent。

硬时限内无可行证书时，不返回未经验证的近似最优动作：有安全持仓则保持/减仓，无安全持仓则 NO_ACTION，必要时 FLATTEN/HALT。任何所谓“超强数学最优”必须落到可复现 gap，而不是模型复杂度。

数值测试包含 manufactured solution、PIDE 与 path simulation 一致性、对偶上下界夹逼、单调性、极限情形和浮点舍入下的硬约束保守性。

## 77. 在线失效监控、日志与新指标

为每个 evidence cell 维护 effect/center/coverage 三类相关变化监控：不是检测任意微小漂移，而是检测是否越过经济相关 corridor。触发后先 Caution/降额，再由确认窗口决定 Broken；恢复同样需要等价性证据。

新增事件：IdentificationSnapshot、FactorSnapshot、MeasurementErrorAudit、SimulatorSetSnapshot、CoverageException、RareEventCertificate、RiskBudgetSnapshot、DROAdversary、SolverCertificate。全部关联 evidence_id、decision_id、model/data/config hash。

模拟器新增指标：prequential log/energy score、PIT 与 coverage、conditional tail coverage、Wasserstein/MMD、extreme co-jump、ruin upper bound、AMS variance、reality-gap decomposition、策略排名跨 simulator family 的 Kendall/Spearman 稳定性。

策略新增指标：robust log-growth、nested-risk cost、ruin/liq/drawdown 上界、safe-capacity utilization、evidence-to-size 单调性、recovery hysteresis、NO_ACTION option value、adversarial regret、solver gap-adjusted value。

核心机制新增指标：raw/orthogonal/IV-EIV 收缩系数、closure incremental effect、confounding flip threshold、measurement-noise sensitivity、官方锚点 placebo dominance 和 effect-corridor breach duration。

全部指标仍按 method × symbol × direction × residual bucket × session phase × regime 输出，并明确区分事实、估计、置信边界、模型最坏值和命名压力值。

## 78. Round 7 门禁与实施顺序

R7-G0 Identification：数据谱系、因子可得性、测量误差防护、负对照和识别假设审计通过。
R7-G1 Simulator Set：S2 每个可信 family 在 lockbox 的路径、条件、尾部与 coverage 达标；不达标模型不得支持策略升级。
R7-G2 Tail Certificate：AMS 与独立估计一致，ruin/liq 上置信界低于预注册容忍度；零失败不按零风险。
R7-G3 Solver：所有硬约束、对偶 gap、离散误差、timeout fallback 和 deterministic replay 通过。
R7-G4 Shadow：只读真实流上 coverage、识别状态、延迟和求解时限连续达标。
R7-G5 Full Matrix：M1–M7 × 7 标的 × factual/strict/stress 全并行；任何版本不得获得更优数据或模拟器。

实施依赖保持：先完成 Round 5 P0–P3；随后 Round 6 H0–H2 与 Round 7 I0（识别样本/因子审计）并行；再完成 S2 credible set、tail engine 和 solver certificate；最后才允许重启完整实验。

Round 7 仍是设计轮：不修改 engine、不编译、不启动实验。下一代码工作不应直接写 M7 仓位公式，而应先实现统一事件、锚点谱系、HypothesisOpportunity 与 simulator calibration split。

## 79. Round 7 研究依据与采用边界

- [Causal DRO duality](https://arxiv.org/abs/2401.16556)：支持 adapted Wasserstein、动态对偶和非前视最坏分布。
- [Risk-averse control by nested risk measures](https://pubsonline.informs.org/doi/10.1287/moor.2022.1314)：支持时间一致风险递归与动态规划。
- [Risk-Constrained Kelly](https://stanford.edu/~boyd/papers/kelly.html)：支持增长目标下显式 drawdown probability bound；仅作规模上界。
- [Distributionally Robust Kelly](https://web.stanford.edu/~boyd/papers/robust_kelly.html)：支持概率分布不确定下最坏 log-growth 和可解凸形式。
- [Multilevel Splitting](https://pubsonline.informs.org/doi/10.1287/opre.47.4.585)：支持高效估计普通 Monte Carlo 难以覆盖的罕见失败概率。
- [Monitoring relevant changes](https://arxiv.org/abs/2509.01756)：支持围绕经济相关 corridor 的长期变化监控，而非检测任意微小变化。
- [Anytime-valid conformal risk control](https://arxiv.org/abs/2602.04364) 与 [regime-weighted VaR calibration](https://arxiv.org/abs/2602.03903)：只作 2026 challenger，必须验证非平稳依赖下 coverage。
- [DML for time series](https://arxiv.org/abs/2603.10999)：只作正交化 challenger；其时间可逆/依赖假设不满足时不得用于正式因果结论。