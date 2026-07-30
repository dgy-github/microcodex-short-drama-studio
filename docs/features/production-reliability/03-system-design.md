# Production reliability system design

Status: G6 implementation and non-network acceptance complete

Rust owns migration, repair, backup/restore integrity, credential storage,
credential audit, and diagnostic redaction. Backup refuses existing restore
targets, rejects links and unsafe relative paths, verifies every SHA-256 before
creating the restore target, and excludes interrupted `.partial` files.

The sidecar persists the validated StartRun payload in its append-only event
log. On restart it resumes accepted non-terminal runs. A completed workflow is
written as `workflow.result.stored` before `run.completed`; the SSE projection
reveals only the result schema/status while the result endpoint can recover the
full artifact after process restart.

The fixed workflow enforces its declared token ceiling while retaining each
artifact. Capability timeout and provider/task failure use stable failure codes
and cannot be degraded into fabricated output.
