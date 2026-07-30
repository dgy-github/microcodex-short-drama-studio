# Advisory fixed story workflow

Status: G5 real integration passed

## Requirements

- `REQ-109`: one accepted `StoryJob` executes the fixed tasks `t01` through
  `t17` in dependency order and produces one `story-package/v1`.
- `REQ-110`: `t11`, `t12`, `t13`, `t14`, and `t16` retain structured review
  records with findings and artifact span references.
- `REQ-111`: the run result is always labelled
  `advisory/non-promotable`; it cannot be used for promotion or release.
- `REQ-112`: provider credentials remain in Rust. Python receives only an
  authenticated, allowlisted capability endpoint.
- `REQ-113`: the end-to-end evidence records every task, agent, artifact hash,
  review record, model route, and terminal state.
- `REQ-114`: the real-run harness receives generation and review endpoint,
  model ID, and credential through process configuration; it does not hard-code
  a provider route or copy a credential into Python.

## Acceptance

- A real configured generation route and review route are invoked and recorded.
- All 17 tasks complete once; every dependency completes before its consumer.
- The final package passes Draft 2020-12 schema validation and product
  reference/episode checks.
- The saved workflow result passes `story-workflow-result/v1`.

The first run may use one provider family for both routes when another account
is unavailable. That proves orchestration and artifact handling, not reviewer
independence or story quality.

## G5 evidence

- Run: `run_af911ce5c25841f1b8ee9e6ddc38bd6f`.
- Result: 17/17 tasks, five retained review records, six episodes, two scripted
  scenes, four characters, and one terminal `run.completed`.
- Provider routes: Qwen `qwen3-vl-plus` for generation and review.
- Artifact: `artifacts/advisory-runs/<run_id>/workflow-result.json`.
- Independent offline Draft 2020-12 validation passed for the package and all
  five review records.
- GLM did not participate: Ark returned `AccountOverdueError`; Zhipu returned
  insufficient balance. The result remains advisory and reviewer independence
  is unverified.
- Run `run_0148aa190ce842c8b103d3885a68dfcb` used the standard DeepSeek
  `https://api.deepseek.com/chat/completions` route with model
  `deepseek-v4-pro` for generation and Qwen `qwen3-vl-plus` for review.
- The second run completed 17/17 tasks and retained five review records, six
  episodes, two scripted scenes, four characters, and `run.completed`; the
  saved workflow result passed the runner's registered schemas and product
  checks.

## Non-goals

- Promotion, release qualification, professional review, unrestricted
  retrieval, desktop UI, video, and automatic skill mutation.
