# Desktop story console system design

Status: G6 implementation and deterministic desktop E2E passed

## Ownership

The desktop crate owns presentation IPC (`CAP-008`, `IFACE-013`) and delegates
domain validation to `story-core`, credential persistence to `story-provider`,
and workflow-result interpretation to the existing artifact contract. It does
not become a second runtime or provider owner.

Svelte invokes only typed Tauri commands. Provider secrets exist in the
password field until submission and in the Rust command argument until the
Credential Manager write completes. Rust returns presence metadata only.

## UI states

- `booting`: load credential presence and completed runs.
- `ready`: edit a draft job, configure providers, select a run.
- `validating`: disable project submission until Rust responds.
- `saving_secret`: clear the password field after success or failure.
- `loading_artifact`: retain the run list while loading detail.
- `error`: show a stable product error without platform or secret detail.

## Artifact boundary

The Rust artifact repository accepts run IDs matching
`run_[a-z0-9]{16,64}`. It canonicalizes the configured root and selected
workflow-result path, rejects path escapes, parses JSON, and requires
`story-workflow-result/v1` with a matching run ID.

## Next slice

`DesktopRunController` composes the existing `CapabilityHost` and
`SidecarProcess`. It loads DeepSeek and Qwen credentials from the Rust
credential owner, starts authenticated loopback processes, and retains one
active run session. Svelte invokes Start, Sync, and Cancel Tauri commands only.
The runtime boundary passes the capability service base URL to Python; the
Python capability client is the single owner that appends
`/v1/capabilities`. Passing a pre-expanded operation path would create a
double-path request and is rejected by the deterministic desktop E2E.

Sync sends the snapshot cursor as `Last-Event-ID`. Rust validates every
`story-agent-event/v1`, ignores duplicate sequences, folds event types into a
`desktop-run-snapshot/v1`, and returns only the newly observed events. A
terminal completion fetches and stores the immutable workflow result.

Python owns only workflow task cancellation and the durable
`run.cancelled` append. Rust remains the process, provider, credential,
artifact, and budget-projection owner.

## Token budget guidance

Svelte presents a non-binding planning recommendation of
`max(180000, 90000 + episodes * 15000)` and updates the entered default when
the user switches between short and long profiles. The submitted
`story-job/v1` remains authoritative, and the sidecar retains the existing
hard fail-closed usage check.
