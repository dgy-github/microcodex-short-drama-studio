# MicrocodeX Short Drama Studio Development Rules

## Boundaries

- This repository owns the short-drama writing product.
- `campaign-muti-agent` is the orchestration dependency.
- nanocodex is an offline skill-analysis tool, not the production runtime.
- Rust owns trusted storage, provider access, rights, budget, and process execution.
- Python sidecars receive typed capabilities only; never unrestricted shell access.
- Svelte must not connect directly to a Python sidecar or model provider.

## Communication

- Long-running work uses asynchronous commands and append-only events.
- Cross-process progress uses authenticated SSE.
- Resume from `Last-Event-ID`; delivery is at least once and consumers deduplicate.
- Connection loss is not a task failure.
- Durable state, approval, artifact, review, policy, and terminal events cannot be dropped.

## Quality

- Story changes must follow `docs/STORY_EVAL_DESIGN.md`.
- LLM judges may filter but cannot promote a model, prompt, graph, or skill alone.
- Production skills are immutable, versioned, and promoted only after hidden human review.
- Never ingest unlicensed protected stories.

## Checks

Run before completion:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
python -m unittest discover -s sidecar -p "test_*.py"
```

