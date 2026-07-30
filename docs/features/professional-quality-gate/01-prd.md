# Professional quality gate infrastructure

Status: G6 non-human infrastructure passed; human execution excluded

## Requirements

- `REQ-127`: construct schema-valid professional-authored discrimination pairs
  without copying protected stories.
- `REQ-128`: compute pair accuracy, blinded professional agreement, confidence
  bounds, and adjudication requirements from shared pair assignments.
- `REQ-129`: create and verify a public commitment for a private holdout without
  exposing its prompts or granting skill-derivation access.
- `REQ-130`: evaluate the nine release gates for model, prompt, graph,
  retriever, skill, and genre-pack candidates.
- `REQ-131`: missing professional review or screenwriter signoff must always
  return `non_promotable`; LLM evidence alone can never return `promote`.

## Scope decision

The user explicitly excluded the human blind test. This feature therefore
implements and verifies the non-human infrastructure only. It does not create
professional identities, hard positives, holdout cases, signoffs, or a real
promotion decision.

## Evidence

- Candidate discrimination-pair construction validates package contracts,
  confounds, distinct content hashes, and evaluation-only rights.
- Holdout sealing detects private-file mutation and refuses an empty set.
- Promotion-gate tests cover passing complete professional evidence, rejection
  after a pairwise loss, required adjudication, and fail-closed
  `non_promotable` output when human evidence is absent.
