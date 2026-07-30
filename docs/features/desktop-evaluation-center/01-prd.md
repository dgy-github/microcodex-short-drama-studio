# Desktop evaluation center

Status: G6 implementation and local integration passed

## Requirements

- `REQ-151`: the desktop lists exactly two evaluation datasets: the versioned
  offline case set under `eval/cases`, and the dynamic online sample set made
  from locally retained advisory runs.
- `REQ-152`: an operator can select one or more eligible cases, or all eligible
  cases, and run an automatic advisory evaluation. Rust performs admission
  checks, invokes the configured Bailian judge without exposing credentials,
  aggregates through `story-eval`, and persists per-case and batch results.
- `REQ-153`: an operator can create one or more blind human assignments from
  either dataset and submit all ten rubric dimensions with reasons and valid
  artifact spans. Blind payloads omit split, generator, prior scores, seeded
  defect keys, and source paths.
- `REQ-154`: selection is bounded and fail-closed. Missing artifacts are shown
  as ineligible, duplicate active work and duplicate blind submissions are
  rejected, and every result remains advisory/non-promotable.
- `REQ-155`: evaluation case summaries remain readable at desktop scale, and
  every row exposes the complete catalog detail through double-click and an
  explicit keyboard-accessible detail action.

## Acceptance

- Svelte calls typed Tauri commands only.
- Partial and all-eligible selection produce the same result contract.
- Offline cases without an archived package remain visible but cannot be scored.
- Online samples are immutable snapshots of completed local workflow results.
- Automatic evaluation reports that it is a single-judge advisory pass, not the
  multi-judge release gate.
- Blind assignments retain the mapping privately while returning only blinded
  review material to the UI.
- Results are atomically persisted below the desktop evaluation data root.
- Case detail opens without changing selection, exposes dataset, genre,
  difficulty, split and eligibility, and closes by button, backdrop or Escape.

## Non-goals

- No hidden professional holdout consumption, promotion, rubric mutation,
  production-policy tuning, unrestricted Python execution, or provider key in
  Svelte/Python.

## G5 evidence

- Catalog integration exposes 30 offline cases with 10 archived eligible
  packages, plus the current local advisory-run set.
- Rust tests cover admission failure, complete judge score aggregation, blind
  metadata removal, artifact-identity redaction and append-only submission.
- Desktop Rust: 13 passed, one paid story-generation E2E ignored.
- Svelte check and production build pass with the evaluation center navigation,
  partial selection, all-eligible actions, result table and ten-dimension blind
  form.

## Unverified

- No paid Qwen evaluation batch was run in this change.
- No actual human reviewer completed a blind assignment; only the deterministic
  contract and persistence path were exercised.
