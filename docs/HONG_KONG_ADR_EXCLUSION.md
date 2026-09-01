# Hong Kong ADR/ADS anchor-stability register

This register is part of the AnchorBell execution boundary.

The purpose is to protect the Hong Kong closing-price anchor. ADR/ADS
existence alone is not a veto: an OTC program with no effective current market
cannot materially reprice the issuer during the Hong Kong close-to-open
interval. The hard rule is instead:

> A Hong Kong-region issuer with active ADR/ADS price discovery during the
> frozen-close interval cannot enter the FrozenClose execution universe.

The decision is issuer-level and fail-closed for unknown market quality. ADR
presence, price-discovery quality, and execution eligibility are separate
facts.

## Hard-excluded instruments

The following reviewed Binance contracts have an ADR/ADS market that is active
enough to be treated as an external price-discovery venue. They are excluded
from the FrozenClose execution universe:

| Binance contract | Hong Kong reference | ADR/ADS evidence |
| --- | --- | --- |
| HK0700USDT | Tencent Holdings | TCEHY |
| TENCENTUSDT | Tencent Holdings | TCEHY |
| HK1810USDT | Xiaomi Corporation | XIACY |
| KUAISHOUUSDT | Kuaishou Technology | KSHTY |
| MEITUANUSDT | Meituan | MPNGY |
| POPMARTUSDT | Pop Mart International Group | PMRTY |

The two Tencent contracts are separate Binance representations of the same
issuer and are both excluded.

Evidence reviewed on 2026-09-01 includes:

- [Deutsche Bank DR directory: Tencent](https://www.adr.db.com/drwebrebrand/dr-universe/dr_details.html?identifier=7592)
- [Deutsche Bank DR directory: Kuaishou Technology](https://www.adr.db.com/drwebrebrand/dr-universe/dr_details.html?identifier=11839)
- [Deutsche Bank DR directory: Pop Mart](https://www.adr.db.com/drwebrebrand/dr-universe/dr_details.html?identifier=11813)
- [OTC Markets: Xiaomi XIACY](https://www.otcmarkets.com/stock/XIACY/overview)
- [OTC Markets: Kuaishou KSHTY](https://www.otcmarkets.com/stock/KSHTY/overview)

## Retained reviewed catalog entries

These entries have ADR/ADS evidence, but the reviewed OTC market is currently
inactive, stale, or without an effective continuous quote. They are retained
for the FrozenClose strategy; the ADR is recorded but never used as the anchor.

| Binance contract | ADR/ADS evidence | Price-discovery classification |
| --- | --- | --- |
| GIGADEVUSDT | GIGDY | NoEffectiveMarket |
| HK0625USDT | SHEIN ADS filing | NoEffectiveMarket |
| MINIMAXUSDT | MMXGY | InactiveOrStale |
| ZHIPUUSDT | KNWAY | NoEffectiveMarket |
| ZHONGJIUSDT | ZHJIY | InactiveOrStale |

The retained status is about current price-discovery quality, not absence of an
ADR/ADS program. A future refresh must re-evaluate the external market before
execution is enabled.

## Enforcement

The Rust catalog exposes separate issuer and market-quality facts:

- `catalog_instruments()` keeps all reviewed entries for audit and review.
- `all_instruments()` returns entries permitted by the FrozenClose policy.
- `instrument_for()` rejects active ADR price discovery and unknown quality.
- `adr_excluded_instruments()` exposes only the active price-discovery set.
- `AdrStatus` records whether an ADR/ADS program exists.
- `AdrPriceDiscovery` records whether it can contaminate the frozen anchor.

The dashboard uses the runtime lookup, so active-ADR symbols cannot be selected
or applied through the UI. A weak or stale OTC ADR is not fed into the anchor
calculation. This boundary is independent of the strategy threshold and cannot
be bypassed by configuration.
## Change protocol

When Binance adds or changes a Hong Kong-region contract:

1. Resolve the underlying issuer and Hong Kong listing.
2. Check active and historical ADR/ADS records, including OTC.
3. Measure the close-to-open ADR quote freshness, two-sided coverage, spread,
   trade frequency, and effective price-discovery behavior.
4. Record the evidence, observation window, and observation date.
5. Set `AdrStatus` and `AdrPriceDiscovery` independently.
6. Exclude only `AdrPriceDiscovery::Active` from FrozenClose.
7. Keep `AdrPriceDiscovery::Unknown` out of live, paper, and testnet execution
   until resolved; weak/stale programs are never fed into the anchor.
8. Run the universe tests and review the exact diff.

This register does not authorize any production order. Testnet and production
remain separately gated by the execution safety policy.
