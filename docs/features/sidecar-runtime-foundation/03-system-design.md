# Sidecar runtime foundation design

Status: G5 process/health slice complete

## Ownership

This extends `story-runtime` (`CAP-002`, `IFACE-002`). Rust owns sidecar process
lifecycle and command readiness. Campaign owns DAG execution, routing, review
and recovery after a command crosses the authenticated typed boundary.

## Lifecycle

The tracked states are `stopped`, `starting`, `ready`, `stopping` and `failed`.
Only `ready` accepts commands. A requested stop is not complete until the
supervised process exits. Any exit while starting or ready is unexpected and
enters failed.

This state machine is independent of the eventual process library so its safety
rules can be tested before process I/O is selected.

## Execution order

`fixed_story_execution_order()` returns the 17 task IDs and dependency edges
from `STORY_MULTI_AGENT_DESIGN.md` §5. The public API exposes a static slice;
there is no constructor for arbitrary task graphs. Validation checks uniqueness
and topological ordering as a repository invariant.

## Process and health integration

Inspection of the pinned upstream commit established that `python -m campaign`
runs a demo, not a server. Upstream
`JsonRpcAgentServer` supplies authenticated JSON-RPC and SSE handlers but does
not bind a listening HTTP process.

The product adapter now supplies an `aiohttp 3.12.15` localhost entry point
around that handler. It rejects non-literal/non-loopback bind addresses and
requires the Rust-provided Bearer token for health and RPC. Port `0` delegates
selection to the OS; the first stdout line is the versioned readiness message.

Rust uses pinned `tokio 1.44.2` process I/O and `reqwest 0.12.15` with rustls.
It accepts readiness only for `127.0.0.1`, verifies authenticated health, and
kills/reaps the child on stop. The token is redacted in Debug and zeroized on
drop.

Run commands, product EventLog replay, `Last-Event-ID` SSE and graceful
application-level shutdown remain later P3b slices.

## Windows background launch

Rust remains the sole process owner. On Windows it applies
`CREATE_NO_WINDOW` when constructing the child process, while preserving null
stdin, piped readiness stdout, suppressed stderr and kill-on-drop lifecycle
control. Other platforms retain their existing launch behavior.
