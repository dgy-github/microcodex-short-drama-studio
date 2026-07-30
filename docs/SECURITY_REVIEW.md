# Security review

Date: 2026-07-28

## Trust boundaries

- Svelte invokes Tauri only and never receives provider credentials.
- Rust owns Windows Credential Manager access, provider HTTP, package
  validation, local storage operations, and process launch.
- Python receives an authenticated typed capability URL/token and a validated
  genre context; it has no unrestricted shell capability.
- Sidecar and capability hosts bind literal loopback addresses with per-launch
  bearer tokens.

## Implemented controls

- Credentials are encrypted at rest by Windows Credential Manager, redacted in
  `Debug`, zeroized on drop, and rotation/deletion is hash-chain audited without
  secret bytes.
- Artifact, revision, approval, event, and workflow-result state is append-only
  or atomically replaced. Restore verifies all hashes before creating a target.
- Provider and task failures fail closed. Token overage and timeout cannot
  create placeholder story output.
- Structured diagnostics redact authorization, tokens, secrets, prompts,
  reasoning, and chain-of-thought.
- Retrieval configuration requires rights evidence and content hashes.
- The owner selected MIT for the pinned `campaign-muti-agent` dependency.
  Revision `1d935714449d18cad5bdc6711a498297ed73a5fb` contains the authoritative
  license and package metadata; the release policy retains and hashes that
  exact license text.

## Open release blockers

- Paid provider degradation has deterministic fault tests but has not completed
  a sustained live-provider soak.

## Accepted personal-release risk

- The owner explicitly accepts unsigned MSI/NSIS installers for this personal
  project. Windows may show an unknown-publisher or SmartScreen warning.
  Authenticode remains available as a future optional public-release control.

No clean-machine release approval is claimed while a blocker remains.
