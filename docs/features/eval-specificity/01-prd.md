# REQ-101 — Dual perturbation specificity

## Problem

`dialogue_subtext` shares a pillar with related character dimensions. The old
specificity value treats expected within-pillar movement and cross-pillar
collateral as the same failure, so P2 cannot distinguish grouping behavior from
true leakage.

## Requirement

Given a scored adversarial pair, when the stage-0 probe writes judge and aggregate
results, then it must output:

- `specificity_all` across every non-target dimension;
- `specificity_cross_pillar` across non-target dimensions outside the target pillar;
- both corresponding collateral-dimension lists;
- backward-compatible `specificity` and `min_specificity` aliases for the all-dimension view.

No provider request is required to upgrade complete saved judge samples.

## Exclusions

- This change does not choose a release threshold.
- This change does not change the current probe status decision.
- This change does not revise pillar membership.
