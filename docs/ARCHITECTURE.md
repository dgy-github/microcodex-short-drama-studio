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
| Offline quality measurement | `story-eval` — absolute, calibrated, gates versions |
| In-run decisions | `story-policy` — relative, cheap, gates nothing permanent |

`story-eval` and `story-policy` are separate crates on purpose. Offline
evaluation is an instrument; online policy is a strategy it validates. The
dependency runs one way: policy configuration is promoted through evaluation,
and evaluation criteria never adapt to production outcomes. See
`docs/ONLINE_POLICY_DESIGN.md`.

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
- Reviewed commit: `1d935714449d18cad5bdc6711a498297ed73a5fb`
- Python import package: `campaign`

The dependency must remain pinned until a deliberate review updates the commit.
