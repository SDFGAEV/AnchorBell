# AnchorBell Dual Environment Runbook

本手册说明 AnchorBell 如何在 Binance Futures Testnet 与 Production 之间切换。
默认永远是 Testnet；Production 必须显式打开。所有命令都只在当前 PowerShell
进程中读取凭证，不把凭证写入仓库、配置文件、日志或回放文件。

## 运行模式

| 模式 | 环境变量 | 凭证变量 | 订单权限 |
| --- | --- | --- | --- |
| Testnet | ANCHORBELL_BINANCE_ENV=testnet 或不设置 | ANCHORBELL_BINANCE_API_KEY/SECRET | 默认关闭 |
| Production 只读 | ANCHORBELL_BINANCE_ENV=production + ANCHORBELL_ENABLE_PRODUCTION=1 | ANCHORBELL_BINANCE_LIVE_API_KEY/SECRET | 关闭 |
| Production 订单 | 在只读配置上再打开订单开关并确认 | 同上 | 独立显式开启 |

程序不会把 Testnet 凭证用于 Production，也不会把 Production 凭证用于 Testnet。
缺少凭证、环境错配或确认不完整时，程序在网络连接前停止。

## Testnet

```powershell
$env:ANCHORBELL_BINANCE_ENV = "testnet"
$env:ANCHORBELL_BINANCE_API_KEY = "<testnet-key>"
$env:ANCHORBELL_BINANCE_API_SECRET = "<testnet-secret>"

cargo run -p static-anchor-engine --bin binance_account_smoke --locked
cargo run -p static-anchor-engine --bin binance_open_orders_smoke --locked
```

两个 smoke 都是签名只读查询：分别调用账户状态和当前挂单查询，不会下单、
撤单或改变账户状态。若只测试公共行情，不需要任何凭证：

```powershell
cargo run -p static-anchor-engine --bin testnet_market_smoke --locked
```

## Production 只读验证

只读验证需要 Production 环境开关和 Production 专用凭证，但不需要订单开关：

```powershell
$env:ANCHORBELL_BINANCE_ENV = "production"
$env:ANCHORBELL_ENABLE_PRODUCTION = "1"
$env:ANCHORBELL_BINANCE_LIVE_API_KEY = "<production-key>"
$env:ANCHORBELL_BINANCE_LIVE_API_SECRET = "<production-secret>"

cargo run -p static-anchor-engine --bin binance_account_smoke --locked
cargo run -p static-anchor-engine --bin binance_open_orders_smoke --locked
```

这两个入口仍然只读，且不会因为设置了 Production 环境就自动下单。
建议先使用只读 API key 验证网络、签名、时间窗口和账户权限。

## Production 订单权限

真实订单需要三个条件同时满足：

```powershell
$env:ANCHORBELL_BINANCE_ENV = "production"
$env:ANCHORBELL_ENABLE_PRODUCTION = "1"
$env:ANCHORBELL_ENABLE_ORDER_SUBMISSION = "1"
$env:ANCHORBELL_LIVE_TRADING_CONFIRMATION = "I_UNDERSTAND_REAL_FUNDS_RISK"
```

还必须提供 ANCHORBELL_BINANCE_LIVE_API_KEY 与
ANCHORBELL_BINANCE_LIVE_API_SECRET。订单传输层会再次检查 policy；没有订单
权限时，order.place 在发送到 Binance 之前被拒绝。现有 generic smoke
入口永远不发送订单，因此不能把 smoke 当作真实下单命令。

真实订单上线前仍须由上层编排器完成 symbol allowlist、交易规则、仓位上限、
maker-only、session flatten、stale-data halt、断线恢复和人工复核。任何未知
订单状态都不得自动增加风险。撤单路径应保持可用，以便在风险门禁触发时退出
挂单。

## 凭证与清理

```powershell
Remove-Item Env:ANCHORBELL_BINANCE_API_KEY -ErrorAction SilentlyContinue
Remove-Item Env:ANCHORBELL_BINANCE_API_SECRET -ErrorAction SilentlyContinue
Remove-Item Env:ANCHORBELL_BINANCE_LIVE_API_KEY -ErrorAction SilentlyContinue
Remove-Item Env:ANCHORBELL_BINANCE_LIVE_API_SECRET -ErrorAction SilentlyContinue
Remove-Item Env:ANCHORBELL_LIVE_TRADING_CONFIRMATION -ErrorAction SilentlyContinue
```

不要把真实 key/secret 放入命令历史、截图、CI 日志、issue、提交或聊天。建议
Production key 关闭提现权限，并只授予当前验证所需的最小权限。

## 证据要求

保存 commit SHA、环境名称、脱敏配置摘要、UTC 时间、symbol、请求 id、HTTP/
WebSocket 状态、错误分类和最终仓位；不得保存 secret、完整认证 payload 或原始
账户响应。Testnet 通过不等于 Production 的流动性、延迟、成交或收益已被证明。

Binance Futures 的 Testnet 与 Production 使用不同的 REST/WebSocket 基地址；
以官方文档为准：
[USDS-M Futures General Information](https://developers.binance.com/en/docs/derivatives/usds-margined-futures/general-info)
和 [WebSocket API General Information](https://developers.binance.com/en/docs/products/derivatives-trading-usds-futures/websocket-api-general-info)。
