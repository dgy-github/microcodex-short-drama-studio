# Architecture

## Ownership

| Boundary | Owner |
| --- | --- |
| Customer job and artifact schemas | Rust product |
| Rights, provider keys, budget, persistence | Rust product |
| Task DAG, routing, review, recovery | Campaign sidecar |
| Progress transport | Append-only events and SSE |
| Desktop presentation | Tauri + Svelte |
| Candidate skill derivation | nanocodex offline |
| Skill promotion | Frozen evaluation plus human blind review |

## Process model

```text
Desktop
  -> Tauri command
Rust StoryService
  -> async command accepted
Campaign sidecar
  -> task events and content deltas
  -> SSE
Rust event consumer
  -> durable projection
  -> typed Tauri events
Desktop
```

Synchronous calls are limited to health, discovery, command acceptance, and
state snapshots. A returned acceptance is not task completion.

## Pinned orchestration source

- Repository: <https://github.com/dgy-github/campaign-muti-agent>
- Reviewed commit: `6f7d0030b127c699ec5b6324b77795ed3a2452e0`
- Python import package: `campaign`

The dependency must remain pinned until a deliberate review updates the commit.

