# AnchorBell Spot Demo Runbook

本手册用于 Binance Spot Demo 的现货接口验证。它与 USDⓈ-M Futures Testnet
使用不同端点、凭证和订单语义，不会改变 AnchorBell 的合约做市主路径。

## 当前边界

- 仅允许 Binance Spot Demo。
- 现货 Demo 端点固定为 `https://demo-api.binance.com/api`。
- 现货 WebSocket API 端点固定为
  `wss://ws-api.testnet.binance.vision/ws-api/v3`。
- maker-only 订单使用 `LIMIT_MAKER`；禁止市价单和主动成交。
- 现货 Demo 凭证不得与 Futures Testnet 凭证混用。
- Production 现货路径未启用，不能由本手册开启。

## 无凭证公共行情烟测

先在远端 PowerShell 验证 Demo REST 可达性和交易对过滤器：

```powershell
$base = "https://demo-api.binance.com/api"
Invoke-RestMethod "$base/v3/exchangeInfo?symbol=BTCUSDT"
Invoke-RestMethod "$base/v3/ticker/bookTicker?symbol=BTCUSDT"
```

该步骤不发送订单，也不需要 API key。响应必须包含 `BTCUSDT` 以及合法的
bid/ask 字符串；网络异常或响应结构未知时应停止。## 认证订单验证

现货 Demo API key 只能在 Binance Demo Trading 页面创建。仅在远端进程环境中
设置，并使用独立变量名：

```powershell
$env:ANCHORBELL_SPOT_DEMO_API_KEY = "<spot-demo-key>"
$env:ANCHORBELL_SPOT_DEMO_API_SECRET = "<spot-demo-secret>"
```

不要把密钥放入源码、文件、命令历史、日志、issue、回放数据或聊天消息。

第一笔订单必须是极小数量、远离盘口的 `LIMIT_MAKER`，并在确认 request id、
exchange order id 和订单状态后立即撤单。若交易所判定订单会立即成交，必须停止。
当前 Rust 核心已经提供独立的 `SpotDemoEndpoints` 和 `SpotOrderWire`，
用于生成签名的 `order.place` 请求；它不会复用 Futures order WebSocket
适配器。

## 证据与禁止事项

保存 commit SHA、脱敏配置、symbol、UTC 时间、request id、exchange order id、
状态序列、撤单结果和最终余额摘要；不得保存 secret 或完整认证载荷。

Spot Demo 通过只证明现货接口、签名、订单生命周期和撤单路径可用，不证明
Futures 流动性、maker 成交概率、策略收益或 Production 安全性。## 参考文档

- [Binance Spot Demo general information](https://developers.binance.com/en/docs/products/spot/demo-mode/general-info)
- [Spot REST trade API](https://developers.binance.com/en/docs/catalog/core-trading-spot-trading/api/rest-api/trade)
- [Spot WebSocket API trade](https://developers.binance.com/en/docs/catalog/core-trading-spot-trading/api/ws-api/trade)
- [AnchorBell Testnet Runbook](TESTNET_RUNBOOK.md)

所有现货 Demo 交易动作都必须经过人工复核，并保持在独立的测试账户内。