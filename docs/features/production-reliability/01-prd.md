# Production reliability and governance

Status: G6 implementation and non-network acceptance complete

## Requirements

- `REQ-139`: credential replacement and deletion produce a secret-free,
  hash-chained audit trail.
- `REQ-140`: incomplete runs recover from the durable command after restart,
  and workflow results become durable before `run.completed`.
- `REQ-141`: story and approval state support version migration,
  interrupted-write repair, integrity-checked backup, and fail-closed restore.
- `REQ-142`: token budget, timeout, provider failure, and bounded concurrency
  fail closed under tests.
- `REQ-143`: diagnostics redact credentials, prompts, and chain-of-thought.
- `REQ-144`: release evidence includes a lockfile-backed dependency inventory,
  security review, incident runbook, and sustained event check.

The pinned `campaign-muti-agent` revision now carries owner-selected MIT
metadata and authoritative license text. The distribution policy binds the
exact revision and retained text by SHA-256.

The desktop now exposes a bounded 3–20 iteration live-provider soak over both
configured routes. It retains only secret-free timing and success/failure
evidence; actual execution still requires rotated user credentials.
