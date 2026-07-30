# REQ-105 — Encrypted provider credential storage

Status: G7 complete

## Requirement

The Windows desktop stores user-supplied model-provider credentials in Windows
Credential Manager through the Rust trusted boundary.

For a validated `(provider, profile)` identity, Rust must support:

- replacing the current secret;
- retrieving it as a redacted, zeroizing value;
- deleting it idempotently;
- distinguishing a missing entry from an unavailable credential store;
- returning errors that never contain the secret or platform error detail.

The tracked repository, project database, logs, events, Svelte frontend and
Python sidecar must never receive persisted credential bytes.

## Exclusions

- Credential rotation history and security audit remain in P9.
- Provider health checks remain in P10.
- Linux and macOS native stores are not release targets in this slice.
- This does not migrate `.env`; evaluation tooling remains separate.

## Acceptance

Unit tests cover identity validation, secret redaction/zeroization boundaries
and error semantics. A Windows-only ignored smoke test writes, reads, replaces
and deletes one uniquely named temporary credential in the real current-user
Credential Manager.
