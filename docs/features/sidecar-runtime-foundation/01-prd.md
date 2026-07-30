# REQ-107 — Sidecar runtime foundation

Status: G5 process/health slice complete

## Requirement

Rust must own a fail-closed lifecycle state machine for the Campaign sidecar and
must expose exactly one fixed 17-task story `ExecutionOrder`.

- commands may be sent only while the sidecar is ready;
- an unexpected process exit becomes failed, not stopped;
- an orderly stop reaches stopped only after process exit;
- every fixed task ID is unique and every dependency points to an earlier task;
- callers cannot supply free-form tasks or dependencies.

## Exclusions

- This slice does not implement run commands, EventLog replay or product SSE.
- It does not register agent implementations or call model providers.

## Acceptance

Rust tests pin the valid lifecycle, reject commands outside ready state, detect
unexpected exit, and validate the complete fixed DAG. A real Windows smoke
starts the installed adapter on an OS-selected loopback port, verifies
authenticated health, then terminates and reaps the child.

## REQ-309 — Background Windows sidecar

On Windows, launching either the development Python sidecar or the bundled
sidecar must not create a visible console window. Readiness and diagnostics
continue through redirected process streams and typed runtime state. Starting a
story keeps the desktop window in the foreground, while stop, health checks and
process reaping retain their existing behavior.
