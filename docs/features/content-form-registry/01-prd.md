# REQ-106 — Content-form asset registry

Status: G7 complete

## Requirement

The runtime must resolve every accepted `StoryJob.content_form` to exactly one
artifact-schema, rubric and case-set manifest triple before a run starts.

For the first release:

- `scripted_short_drama` resolves to the existing `story-package/v1` schema,
  `judge-v1` rubric and `eval-v0.1.0` case-set manifest;
- registry paths are repository-relative portable asset references;
- duplicate forms, unknown registry schemas and unsafe paths are rejected;
- a job cannot override any member of the triple.

## Exclusions

- No knowledge/explainer or real-creator assets are added.
- The registry does not freeze or promote the advisory evaluation contract.
- It does not start the Python sidecar or load provider credentials.
- It does not change aggregation, gating or verdict logic.

## Acceptance

The checked-in registry resolves the real scripted-drama job to three existing
assets. Unit tests reject duplicate form entries, path traversal and an
unregistered form.
