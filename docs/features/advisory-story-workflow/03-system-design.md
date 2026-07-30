# System design

Rust extends `CAP-003` with an authenticated loopback capability host. It owns
Qwen/GLM credentials and provider HTTP calls. The host accepts only
`generate_structured_text` and `validate_artifact`; its launch token is
independent from the sidecar token.

Python extends `CAP-004`. `RunService` starts a background
`AdvisoryStoryWorkflow` only when a capability endpoint is supplied. The
workflow builds Campaign `ExecutionOrder`, `Task`, and `AgentSpec` objects from
the product-owned fixed template. Specialized Campaign agents call the typed
Rust capability, retain task artifacts in run context, and emit product events.

The generation and review endpoint, model identity, and credential are process
configuration consumed by Rust; only model identities are retained in the
result. Common provider severity labels are normalized at the Python contract
boundary to the closed `critical/major/minor/note` vocabulary. The initial
fallback may use Qwen for both when the GLM account is unavailable; the artifact
remains non-promotable. `t02` produces an empty authorized retrieval manifest
and performs no network retrieval. `t15`
produces the revised complete package, `t16` records final review, and `t17`
asks Rust to validate and content-address the package.

The integration runner starts both localhost processes, submits one immutable
job, waits for `run.completed`, validates the workflow result, and writes
ignored evidence under `artifacts/advisory-runs/<run_id>/`.

Failure is closed: an unavailable provider, malformed JSON, missing task,
non-passing final review, invalid package, or missing review record emits
`run.failed` and no successful workflow result.
