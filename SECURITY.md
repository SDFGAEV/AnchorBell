# Security Policy

Do not report credentials, private keys, signed requests, account identifiers, or
sensitive replay data in public issues. Remove secrets from logs before sharing.

## Operational rules

- Use Binance testnet credentials for development.
- Keep secrets in environment variables or an external secret manager.
- Production order permission must be explicitly enabled.
- Treat unknown exchange effects as unresolved until reconciled.
- Do not bypass post-only, position, stale-data, or session-close gates.

For a suspected vulnerability, contact the repository owner privately before
publishing details. Include affected commit, component, reproduction, and impact
without including secret material.
