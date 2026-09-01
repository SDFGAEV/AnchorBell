# Hong Kong ADR/ADS exclusion register

This register is part of the AnchorBell execution boundary.

The hard rule is:

> A Hong Kong-region issuer with an active or historical ADR/ADS program
> cannot enter the AnchorBell execution universe.

The rule is issuer-level, includes OTC depositary receipts, and is fail-closed:
an unknown or contradictory status is not tradable.

## Excluded instruments

The following reviewed Binance contracts are excluded from execution because
their underlying issuer has an ADR/ADS representation:

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

The current reviewed catalog retains these seven Hong Kong-region instruments
after the ADR/ADS review:

| Binance contract | Instrument class |
| --- | --- |
| CSOPSAMSUNG2LUSDT | HK-listed leveraged product |
| CSOPSKHYNIX2LUSDT | HK-listed leveraged product |
| GIGADEVUSDT | HKEX 3986 H-share |
| HK0625USDT | HKEX 0625 |
| MINIMAXUSDT | Hong Kong-listed issuer |
| ZHIPUUSDT | Hong Kong-listed issuer |
| ZHONGJIUSDT | HKEX 3308 H-share |

The retained status means that no ADR/ADS was identified in the reviewed
registry for the current catalog snapshot. It is not a permanent assertion
about future issuance. Any future catalog refresh must revalidate the issuer
before enabling it.

## Enforcement

The Rust catalog exposes two separate views:

- `catalog_instruments()` keeps all reviewed entries for audit and review.
- `all_instruments()` returns only entries with ConfirmedAbsent status.
- `instrument_for()` is the runtime lookup and rejects ConfirmedPresent and
  Unknown statuses.
- `adr_excluded_instruments()` exposes the excluded set for tests and audits.

The dashboard uses the runtime lookup, so excluded symbols cannot be selected
or applied through the UI. This boundary is independent of the strategy
threshold and cannot be bypassed by configuration.
## Change protocol

When Binance adds or changes a Hong Kong-region contract:

1. Resolve the underlying issuer and Hong Kong listing.
2. Check active and historical ADR/ADS records, including OTC.
3. Record the evidence and observation date.
4. Set ConfirmedAbsent, ConfirmedPresent, or Unknown.
5. Run the universe tests and review the exact diff.
6. Keep Unknown out of live, paper, and testnet execution until resolved.

This register does not authorize any production order. Testnet and production
remain separately gated by the execution safety policy.
