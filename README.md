# MicrocodeX Short Drama Studio

Windows-first short-drama story development workspace. It turns a short Chinese
idea into multiple story candidates, character and episode plans, reviewed
sample scenes, and a versioned story package.

## Architecture

- Rust workspace: trusted product contracts, event protocol, storage/provider seams, and evaluation helpers.
- Campaign sidecar: multi-agent DAG orchestration, routing, review, recovery, and event replay.
- Tauri + Svelte desktop: operator UI; added after the M0 runtime contract is stable.
- nanocodex: offline derivation of candidate `SKILL.md` files from licensed human revision traces.

Long-running communication follows one rule:

```text
asynchronous command -> append-only event -> SSE -> durable replay
```

See:

- [Story multi-agent design](docs/STORY_MULTI_AGENT_DESIGN.md)
- [Story evaluation design](docs/STORY_EVAL_DESIGN.md)
- [Architecture decisions](docs/ARCHITECTURE.md)

## Current milestone

M0 establishes schemas and executable protocol foundations. It does not claim
to generate production stories yet.

## Development

```powershell
cargo test --workspace --all-features
python -m unittest discover -s sidecar -p "test_*.py"
```

