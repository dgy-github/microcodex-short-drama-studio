# REQ-315..319 - Multi-agent communication reliability

Status: G2 contracts ready

## Requirements

### REQ-315 - One run identity

Every durable event produced while executing an accepted story run uses the
`run_id` returned by `StartRun`. The Campaign runtime's private execution
identity must never leak into the product event stream.

### REQ-316 - One terminal decision

Completion, failure and cancellation compete through one per-run serialization
boundary. Exactly one of `run.completed`, `run.failed` or `run.cancelled` is
durably appended, including under concurrent Cancel calls and completion races.

### REQ-317 - Artifact checkpoint recovery

A restarted sidecar restores completed task artifacts from trusted,
content-addressed storage and executes only the first incomplete task and its
downstream dependants. A missing, corrupt or incomplete checkpoint is not
trusted and is recomputed. Previously consumed tokens remain charged.

### REQ-318 - Durable replay followed by live subscription

An SSE connection first replays durable events after `Last-Event-ID`, then
follows the EventLog subscription without 100 ms database polling. Sequence
gaps and subscriber overflow are repaired from the durable log. Heartbeats do
not advance the durable cursor.

### REQ-319 - Typed artifact handoff

The fixed in-process DAG uses synchronous typed artifact handoff. Agent output
is retained through an authenticated Rust capability before
`task.artifact.ready`; events and recovery checkpoints carry only schema, hash
and `content_ref`. A message broker and free-form peer chat are excluded.

## Acceptance

- one accepted run ID appears on every projected event;
- concurrent Cancel/Cancel and Cancel/Complete races retain one terminal event;
- a recovered run does not call providers for restored tasks;
- SSE receives live events without periodic replay and repairs an injected gap;
- artifact bytes are written and verified by Rust-owned storage, while task
  events contain references instead of complete artifacts.

## Exclusions

- Remote third-party agents and distributed brokers;
- changing Svelte to connect directly to the sidecar;
- resumable provider requests whose upstream provider offers no idempotency API;
- removing the final validated package from the completed workflow result.
