# Agent communication reliability design

Status: G2 contracts ready

## Ownership and communication model

Rust remains the trusted owner of storage, provider access, process execution
and product projection. Python owns the fixed Campaign DAG. Within one Python
process, the coordinator synchronously dispatches typed tasks and downstream
tasks consume immutable artifacts only after `task.artifact.ready`. EventLog is
the audit, recovery and cross-process progress channel; it is not a peer-chat
mailbox. Cross-process commands remain authenticated HTTP and progress remains
authenticated SSE.

## Run identity and terminal arbitration

The pinned Campaign runtime currently allocates a private execution ID. A
run-scoped EventLog adapter normalizes Campaign events to the accepted product
`run_id` and maps Campaign's internal lifecycle completion to non-terminal
orchestration events. `RunService` alone appends product terminal events.

Each run has one async terminal lock. Completion, failure and cancellation
replay the run's durable tail while holding this lock and append only when no
terminal record exists. Repeated contenders return the existing terminal
record.

## Artifact retention and recovery

`story-storage` owns an immutable content-addressed file store. The authenticated
Rust capability host exposes only `retain_artifact` and `load_artifact`; the
sidecar receives no path or shell capability. Content references use
`artifact://sha256/<lowercase hex>` and reads re-hash bytes before returning.

The sidecar keeps decoded artifacts in run-local memory for prompt construction.
After restart it intersects `task.artifact.ready` with `task.completed`, loads
and verifies each artifact in topological order, and restores only a dependency-
closed prefix/subgraph. Pending tasks are submitted to Campaign with dependencies
on already restored tasks removed. Token usage is restored from all durable
completion events before new provider work begins.

## SSE replay and subscription

The server registers a bounded EventLog subscription before taking its replay
snapshot. It tracks a private global scan cursor as well as the delivered
per-run cursor. After replay it consumes subscription events. A sequence jump,
heartbeat interval or subscriber overflow triggers replay from the global scan
cursor; duplicate records are ignored. This preserves at-least-once delivery
without a database query every 100 ms.

## Failure handling

- artifact retain failure prevents `task.artifact.ready`;
- corrupt or missing artifact content causes that task and downstream tasks to
  rerun, never silent reuse;
- a dropped subscription event is repaired from SQLite;
- disconnect only closes the SSE consumer and never changes run state;
- terminal arbitration is fail-closed and produces one durable winner.
