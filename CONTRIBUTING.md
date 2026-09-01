# Contributing to AnchorBell

AnchorBell is a focused open-source research and execution engine. Contributions
should preserve the separation between deterministic strategy/risk logic and
external exchange adapters.

## Before opening a pull request

- Explain the ownership boundary and invariant being changed.
- Add focused regression tests for behavioral changes.
- Update the owning documentation.
- Run `cargo fmt --all -- --check` and `cargo test --workspace --locked`.
- Never include API keys, account data, or authenticated payloads.

Keep commits narrow and describe the exact verification performed. Changes that
weaken maker-only execution, session flattening, stale-data handling, or
production safety require explicit design discussion.
