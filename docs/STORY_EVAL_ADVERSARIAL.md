# Adversarial Set Construction

Version: `story-eval-adversarial/v1.0.0` — see [`VERSIONS.md`](VERSIONS.md)
Governs: `eval/adversarial/pairs.jsonl`,
`schemas/eval-adversarial-pair-v1.json`.

Scope: how hard negatives, hard positives, and minimal perturbation pairs are
built, validated, and retired. Consumed by
[`STORY_EVAL_V1.md`](STORY_EVAL_V1.md) section 7.

> **v1 scope.** This document describes the full target method. v1 builds only
> seeded pairs and minimal perturbation pairs, both derived from a generated
> baseline. Hard positives, and therefore true discrimination pairs, are
> deferred: nobody available in v1 can author a credible hard positive, and a
> pair whose positive is merely competent measures nothing. Sections 2, 4, 6, 7,
> and 8 apply to v1 as written. Sections 1, 3, 5, and 9 describe the target
> state; where they assume a professional human on the measuring side, v1
> substitutes the internal spot check and treats every resulting number as a
> lower bound.

## 1. Hardness is measured, not authored

A hard negative is not a bad story. It is an artifact where an evaluator scores
high and a competent human scores low. Hardness is therefore a property of the
pair (artifact, evaluator), measured after construction. An artifact everyone
correctly rejects is an ordinary negative and does not enter the set.

Consequence: authoring is not the deliverable. Authoring plus measurement plus
an accept/reject decision is the deliverable.

## 2. Masking recipes

A hard negative pairs one surface virtue that evaluators reward with one defect
that should disqualify. Recipes are enumerable and map onto the parent failure
taxonomy.

| Masking virtue | Hidden defect | Why it survives |
| --- | --- | --- |
| dense per-episode cliffhangers | `HOOK_FAKE` | automatic checks count hooks, not whether the next episode carries the consequence |
| symmetric setup and callback | false payoff | setup/payoff reference integrity passes while the payoff carries no accumulated pressure |
| high emotional intensity, direct confrontation | `EMOTION_UNEARNED` | intensity reads as emotional progression |
| every line quotable | `VOICE_COLLAPSE` | one strong authorial voice reads as good dialogue |
| complete character cards, stated motives | `MOTIVE_EXPLICIT` | clarity reads as credibility; the most counter-intuitive and most effective recipe |
| fast turns, many reversals | `PLOT_CONVENIENCE` | coincidence reads as pacing |
| dense genre markers | `TROPE_STACK` | reads as genre fulfilment |

Single-defect negatives are the default. Only they attribute failure to a
dimension, which is what `defect_localisation` and perturbation metrics need.
Multi-defect negatives test the overall gate. Target ratio 2:1.

## 3. Sources

| Source | Cost | Strength | Risk |
| --- | --- | --- | --- |
| A. authored from scratch | high | most natural masking | authors trained to write well tend to over-signal the flaw |
| B. targeted degradation of a known-good artifact | low | perfect minimal pairs, confounds aligned by construction | same author for positive and negative may leave a detectable signature |
| C. harvested from real runs | near zero | hits the system's actual blind spots, self-renewing | requires runs to exist |

M0 has no runs, so source C is unavailable. **v1 uses B as the bulk and A for
flagship samples.** Source C becomes primary as soon as the first runs land; its
selection rule is `judge_score - human_score` above a threshold, which is why
that delta must be persisted per artifact from the very first run.

The author of a negative never reviews it.

## 4. Author constraints

Binding. A delivery violating any of these is rejected without review.

1. **Must pass every admission gate.** A negative that trips schema, constraint,
   repetition, or reference-integrity checks never reaches the judge and
   measures nothing. The bar is: automatic checks find nothing wrong.
2. **No safety or rights violation**, or it is rejected for the wrong reason and
   the measurement is contaminated.
3. **The defect must be load-bearing.** Repair must require `scene_rewrite` or
   `restructure`. If a `line_edit` fixes it, it is cosmetic and out of scope.
4. **Surface metrics at or above the incumbent median** — hook count, reversal
   count, dialogue ratio. This is the operational definition of "attractive".
5. **A defect key must accompany delivery**: exact spans plus `problem_code`.
   This is the ground truth for judging whether an evaluator was right for the
   right reason. Without it, every detection metric is inflated by luck.
6. **Confound matching with the paired positive**: episode count, word count,
   scene count, format. Otherwise the measurement captures format detection
   rather than story judgement.
7. Authors are not told the automatic-check thresholds, or they will write to
   the boundary.

## 5. Acceptance

Each candidate is run through admission gates, the judge set, and two blind
internal reviewers who are not the author.

|  | human scores low | human scores high |
| --- | --- | --- |
| **judge scores high** | accept as hard negative | defect does not hold — the taxonomy entry is questionable, archive for review |
| **judge scores low** | ordinary negative, retained as a sanity check only | construction failed, discard |

Additional acceptance condition: at least one reviewer independently located the
seeded span. A low score with no located defect means something else caused it
and the sample is not clean.

```text
hardness = judge_score - human_score
```

In v1 the human side is the internal spot check, not a screenwriter. This weakens
the upper-left quadrant: internal reviewers may miss defects a screenwriter would
catch, so v1 hardness is a lower bound. Recorded as such; re-measured when the
professional slot is filled.

## 6. Minimal perturbation pairs

One targeted edit on a known-good artifact, everything else byte-identical.

| Perturbation | Target dimension |
| --- | --- |
| replace a subtextual line with a direct statement of motive | 台词与潜台词 |
| insert a coincidence that resolves existing pressure | 因果一致性 |
| transfer one character's coping behaviour to another | 人物区分度 |
| move an emotional turn earlier, removing its accumulation | 情感推进 |
| replace an episode-ending consequence with an unresolved tease | 短剧节奏 |
| add a location or cast member beyond the production tier | 可制作性 |

Yields sensitivity (the targeted dimension moved) and two specificity views:

- `specificity_all`: every non-target dimension, including the target pillar;
- `specificity_cross_pillar`: only dimensions outside the target pillar.

Both are retained. Low cross-pillar specificity means the evaluator is giving
a holistic impression score; the gap between the two views is evidence for the
later pillar-grouping review.

Prior art check pending: perturbation-based negative construction appears in the
OpenMEVA / UNION line of work. If their operator list is applicable, reuse it
and author only the short-drama-specific operators above.

## 7. Anti-patterns

- **No LLM-generated hard negatives in v1.** Same-family generation and judging
  produce correlated blind spots; the resulting hardness number is
  uninterpretable in either direction.
- **Negatives must not be longer than their positives.** Length correlates with
  judge score and is the most common hidden confound.
- **Do not fix the author roster.** One author's notion of "attractive" becomes
  a learnable signature within a few rounds.

## 8. Leakage and rights

- Hard negatives in `holdout` and `challenge` carry `allowed_uses:
  ["evaluation"]` only, never `skill_derivation`. Otherwise skill generation
  learns to avoid these specific samples rather than this class of defect.
- Defect keys are stored separately from artifacts and never enter generation
  context.
- The set rotates on the parent contract's quarterly `challenge` refresh. A
  fixed negative set is Goodharted within a few iterations. Retirement rules are
  not yet defined and are tracked as open.

## 9. Staged rollout

| Stage | Size | Cost estimate | Decision |
| --- | ---: | --- | --- |
| 0. single-case probe | 1 | ~0.5 day | does the masking recipe produce a measurable gap at all |
| 1. masking-path probe | 7 | ~3 days | which of the seven recipes actually survive the judge |
| 2. production | 15–40 pairs | 15–35 days | size set by stage 1 result |
| 3. steady state | rolling | low | harvesting takes over |

Stage 1 uses **one shared base positive** degraded seven ways, so the masking
path is the only variable.

Stage 1 branches:

| Result | Response |
| --- | --- |
| 4+ recipes effective, large gap | proceed to 40 pairs |
| 1–2 effective | cut to 15 pairs, move the saved effort to perturbation pairs |
| none effective | check whether degradation was load-bearing; if it was, the judge is stronger than assumed and that conclusion itself needs recording |
| judge high but internal reviewers disagree with each other | stop; the problem is reviewer calibration, not the dataset |

Cost estimates are unvalidated. Review effort, not authoring effort, is expected
to dominate: each pair needs two blind reviews of both members.

Stage 0 and stage 1 exist so that roughly three days of work decides whether to
spend the rest.
