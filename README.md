# MicrocodeX Short Drama Studio

[GitHub repository](https://github.com/dgy-github/microcodex-short-drama-studio)

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

Quality is governed by **two separate design systems**. Offline evaluation is the
instrument that gates versions; online policy is the strategy that makes
in-run decisions. The dependency runs one way — policy configuration is promoted
through evaluation, and evaluation criteria never adapt to production outcomes.

| Document | Version |
| --- | --- |
| [Offline evaluation v1](docs/STORY_EVAL_V1.md) — runnable now, no professional panel | `story-eval-offline/v1.0.0` |
| [Adversarial set construction](docs/STORY_EVAL_ADVERSARIAL.md) | `story-eval-adversarial/v1.0.0` |
| [Online policy design](docs/ONLINE_POLICY_DESIGN.md) — in-run weighting | `online-policy-design/v1.0.0` |
| [Story evaluation target contract](docs/STORY_EVAL_DESIGN.md) — professional-panel end state | `story-eval-target/v1.0.0` |
| [Story multi-agent design](docs/STORY_MULTI_AGENT_DESIGN.md) | `story-multi-agent/v1.0.0` |
| [Architecture decisions](docs/ARCHITECTURE.md) | `architecture/v1.0.0` |

Version bump rules, artifact bindings, and current freeze state are in
[docs/VERSIONS.md](docs/VERSIONS.md). Nothing is frozen yet.

Phase sequencing and exit criteria are in [docs/ROADMAP.md](docs/ROADMAP.md);
where the last session stopped is in [HANDOFF.md](HANDOFF.md).

## Current milestone

P5-P10 non-human implementation is present as an advisory desktop release
candidate. It generates and reviews story packages, but does not claim stable
release or model/prompt promotion while the blockers in `CHANGELOG.md` remain.

## Development

首次开始开发或复制/移动仓库后：

```powershell
python scripts/init_project.py --check
python scripts/init_project.py --name "MicrocodeX Short Drama Studio" # 仅未初始化时
```

开发前先查询 `docs/project-memory/PROJECT_REGISTRY.yaml`，跨层功能按
`docs/development/WORKFLOW.md` 的 G0-G7 门禁推进。

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
python -m unittest discover -s sidecar -p "test_*.py"
python -m unittest discover -s eval/tools -p "test_*.py"
python scripts/init_project.py --check
```
