# Binance Static Anchor Arbitrage Architecture

## Design principles

1. High cohesion, low coupling.
2. Research components are separated from execution components.
3. Data collection never contains strategy logic.
4. Strategy consumes typed market snapshots only.
5. Backtest and live trading share the same interfaces.

## Layers

```
Exchange Adapter
        |
        v
Market Data Domain
        |
        v
Research Models
        |
        v
Signal Engine
        |
        v
Execution Engine
        |
        v
Risk Controller
```

## Core invariant

The alpha hypothesis is:

Binance perpetual traded price temporarily deviates from Binance index price during A-share market closure.

The system studies passive maker opportunities around this deviation.
