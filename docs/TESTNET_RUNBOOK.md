# AnchorBell Testnet Runbook

本手册用于第一次真实 Binance Futures Testnet 验证。它不授权 Production，也不代表可以无人值守运行。

## 当前边界

- 只允许 Binance USDⓈ-M Futures Testnet。
- 生产环境默认关闭；任何环境错配都会在网络连接前 fail-closed。
- 只允许 `LIMIT + GTX` 被动订单；禁止市价单和主动成交。
- 测试凭证只能通过进程环境注入，不得写入文件、日志、回放数据或提交。
- 未提供凭证时，程序必须停在缺少凭证状态。

## Latest public-market smoke evidence

2026-09-01 在远端 Windows 上执行 `testnet_market_smoke`，Rust 适配器在 5 秒连接门禁内返回 `ConnectTimeout`，未收到任何行情事件。随后独立验证显示：

- `https://demo-fapi.binance.com/fapi/v1/time` 可访问；
- PowerShell 原生 WebSocket 可连接 `wss://demo-fstream.binance.com/public/stream`，并收到 BTCUSDT `bookTicker`；
- 因此本次失败不是凭证缺失或 Binance Testnet 服务整体不可用，而是该运行时的 Rust TCP/TLS 网络路径尚未闭合。

在该 blocker 解决并重新取得事件证据前，P4 不得标记为通过，不能发送任何订单。

## 运行前检查

1. 使用专用 Testnet API key，只开启读取与交易所需的最小权限。
2. 在测试配置中填写明确的 symbol allowlist、tick/step、最小名义金额、仓位上限和 session 窗口。
3. 仅在远端 PowerShell 进程中设置凭证：

```powershell
$env:ANCHORBELL_BINANCE_API_KEY = "<testnet-key>"
$env:ANCHORBELL_BINANCE_API_SECRET = "<testnet-secret>"
```

不得把上述值放入 `.env`、源码、命令历史、issue、日志或录屏。

## 分阶段验证

### A. 无凭证公共行情烟测

先验证 Testnet public market WebSocket、bookTicker/markPrice 解码、ping/pong、帧大小限制和重连。此阶段不得发送订单。

### B. 认证但零风险请求

验证凭证加载、签名 canonicalization、账户/订单查询和时间窗口错误处理。只允许读取类请求；任何未知响应都不能生成成交事件。

### C. 单笔 maker 订单

使用极小数量和明确的 allowlisted symbol：提交一笔远离盘口的 `LIMIT + GTX`，立即执行 cancel，核对 request id、exchange order id、本地生命周期和最终 remote status。若被判定会立即成交，必须停止测试。

### D. 受控成交与恢复

只有 A-C 全部有证据后，才可分别验证 partial fill、full fill、断线恢复、重启 reconciliation、stale-data halt 和 session flatten。每个场景单独运行，发生未知状态立即停止。

## 证据要求

每个场景保存：commit SHA、配置摘要（脱敏）、symbol、UTC 时间、请求 id、exchange order id、订单状态序列、风险状态、重连次数和最终仓位。不得保存 API secret 或完整认证 payload。

## 明确禁止

- 不得把 Testnet 通过等同于 Production 可用。
- 不得在没有人工复核的情况下开启 Production。
- 不得用回测成交结果证明真实流动性或收益。
- 不得因网络、订单或账户状态不确定而自动增加风险。
