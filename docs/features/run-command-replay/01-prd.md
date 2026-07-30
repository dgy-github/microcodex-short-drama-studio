# REQ-108 — Idempotent StartRun and event replay

Status: G5 integration passed

## Requirement

The authenticated sidecar accepts `StartRun` asynchronously and exposes its
durable events over SSE.

- `Idempotency-Key` is required and scoped to the command endpoint;
- repeating the same key and identical command returns the original acceptance
  record without appending another `run.accepted` or `task.queued`;
- reusing the key for different input returns conflict;
- acceptance queues only `t01` from the fixed execution order;
- `Last-Event-ID=N` replays only matching-run events with `seq > N`;
- replay is ordered, at least once, and ends its historical prefix with a
  non-durable `replay-complete` SSE comment.

## Exclusions

- No task is executed after `t01` is queued in this slice.
- ResumeRun, CancelRun and SubmitHumanInput remain later commands.
- No Svelte client or Tauri projection is added.
- The advisory evaluation state is unchanged.

## Acceptance

A real Rust-to-Python integration test sends the same `StartRun` twice, receives
the same acceptance record, observes exactly two historical events, reconnects
with the first sequence as `Last-Event-ID`, and receives only the second event.

## REQ-310 — Desktop stale-run reconciliation

Before accepting another desktop Start, Rust replays all events after the local
cursor. A terminal event releases the single-run gate; a genuinely active run
returns its current snapshot without starting duplicate work or showing a
misleading `run_active` error.
