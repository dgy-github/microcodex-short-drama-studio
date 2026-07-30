# REQ-103 — Local Codex supplemental judge

Status: G7 complete

## Requirement

The stage-0 probe can use the authenticated Codex CLI on the operator's machine
as a third, non-Chinese-native judge family. Each sample must:

- pin an explicit model and remain disjoint from the generator family;
- run as an ephemeral session in a read-only sandbox;
- start outside the repository so project instructions and files are not judge
  inputs;
- receive the pair only through stdin and return the tracked JSON schema;
- retain provider usage when the CLI emits it;
- carry a fingerprint of the supplemental judge configuration.

The model is `gpt-5.4`: it is present in the local catalog and works with the
installed standalone CLI. The newer desktop default `gpt-5.6-sol` requires a
newer CLI and is not silently substituted.

The existing Qwen and GLM results remain reusable because their input
fingerprint already covers `eval/judges.json`; the supplemental Codex
configuration is tracked and fingerprinted separately.

## Exclusions

- Codex is an offline evaluator, not a production model provider.
- The repository does not store or export the operator's Codex credentials.
- A third LLM judge does not satisfy hidden professional review.

## Acceptance

One full stage-0 result contains three judge families and six validated Codex
samples (three forward and three reverse), with no missing or invalid spans.
