# Live provider soak

Status: G6 implementation and local integration passed

## Requirements

- `REQ-159`: a desktop operator can explicitly run a bounded live soak against
  both configured production provider routes without a developer console.
- `REQ-160`: Rust owns credentials, route resolution, request execution,
  concurrency exclusion, latency aggregation, and immutable result storage.
- `REQ-161`: the result reports success/failure counts and latency summaries
  without provider secrets, prompts, response bodies, endpoint URLs, or raw
  network errors.

## Acceptance

- One soak performs the selected 3–20 iterations for both DeepSeek and Bailian.
- Both credentials and both routes are resolved before the first paid request.
- A second concurrent soak is rejected.
- Every request asks for and validates the same minimal JSON health object.
- Provider failures are counted as `degraded`; they do not fabricate success.
- One complete `provider-soak-result/v1` is atomically retained below the local
  application data root.
- Svelte exposes a deliberate button, cost warning, busy state, and result
  summary through typed Tauri IPC.

## Non-goals

- No automatic background billing, secret echo, response-content retention,
  model promotion, human evaluation, or direct Svelte/Python provider access.

## Evidence

- Rust tests cover iteration bounds, latency/count aggregation, redaction,
  schema validation, immutable persistence, and partial-file cleanup.
- Desktop Rust strict Clippy and 19 non-paid tests pass.
- Svelte check and production build pass with the explicit paid-action warning,
  bounded iteration input, disabled preflight state, and result summary.
- Live execution remains pending until both rotated credentials are configured.
