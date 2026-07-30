# StartRun and replay design

Status: G5 integration passed

## Ownership

Rust `story-runtime` (`CAP-002`) validates the `StoryJob`, owns the launch token
and calls the sidecar. The Campaign adapter (`CAP-004`) owns command
deduplication and its operational EventLog. Rust remains the owner of product
state and durable projection.

## Command contract

`POST /v1/runs` accepts `start-run-command/v1` plus an `Idempotency-Key` header.
The key is 16–128 printable ASCII characters. The command contains the complete
`story-job/v1`; it cannot override the fixed execution order.

The first request appends `run.accepted` and one `task.queued` for `t01`, then
returns `story-command-acceptance/v1`. The acceptance record and a canonical
SHA-256 command fingerprint are retained in the `run.accepted` event. A repeat
scans the EventLog under an async lock:

- same key and fingerprint: return the stored record, append nothing;
- same key and different fingerprint: HTTP 409.

## SSE replay

`GET /v1/runs/{run_id}/events` requires the same Bearer token. The sidecar calls
Campaign `EventLog.replay(since)` and filters by `run_id`. Each durable record is
projected into `story-agent-event/v1`; the Campaign sequence is the SSE `id`.

After all current historical events, the server emits the SSE comment
`: replay-complete`. This does not advance sequence. The stream then polls the
durable EventLog for later records and sends heartbeat comments without
converting disconnect into run failure.

## Storage

The sidecar uses the pinned Campaign `SqliteEventLog` in the exact run workspace
selected by Rust. It is operational orchestration state, not a second product
artifact database. The real smoke uses a disposable directory under `target`.

## Desktop reconciliation

`DesktopRunController` serializes Start calls with one async gate. While holding
that gate it refreshes the current projection through the existing
`Last-Event-ID` replay path. Terminal sessions no longer block the next run.
If the prior session is still active, the existing snapshot is returned and no
provider host or second sidecar is created.
