# Short-Drama Evaluation v1 (No Professional Panel)

Version: `story-eval-offline/v1.0.0` — see [`VERSIONS.md`](VERSIONS.md)
Status: executable first version for M0; advisory, does not gate promotion.
Parent contract: [`STORY_EVAL_DESIGN.md`](STORY_EVAL_DESIGN.md).
Counterpart system: [`ONLINE_POLICY_DESIGN.md`](ONLINE_POLICY_DESIGN.md).
Governs: `eval/manifests/eval-v0.1.0.json`, `eval/rubrics/judge-v1.yaml`,
`crates/story-eval`.

`STORY_EVAL_DESIGN.md` describes the target state, which requires a professional
screenwriter panel. This document describes what is actually runnable now,
before any screenwriter is engaged. Where v1 deviates from the parent contract,
the deviation is stated explicitly and marked as temporary.

## 1. What v1 is and is not

v1 measures two things:

1. the **quality of generated artifacts**, scored by LLM judges;
2. the **defect detection capability of the evaluation apparatus**, measured on
   artifacts with deliberately seeded defects.

The second is the primary deliverable, and its scope is narrower than it may
appear. Detecting a seeded defect is not the same as telling a good story from a
plausible-looking bad one:

| v1 can establish | v1 cannot establish |
| --- | --- |
| whether an evaluator notices a defect that was deliberately introduced | whether an evaluator can rank genuine quality |
| whether it locates that defect in the right place | where the quality ceiling is |
| whether each dimension responds to the defect it is supposed to own | whether scores align with professional taste |

The reason is a capability limit, not a design choice: a true discrimination
pair needs a **hard positive** — a genuinely good, deliberately unflashy
artifact — and nobody available in v1 can write one. Without a trustworthy
positive, both members of a pair are merely competent, and a preference between
them means nothing. Genuine quality discrimination is therefore deferred to the
professional version, together with the `pair_accuracy` metric that expresses
it.

What remains is still worth building. If a rubric dimension does not move when
its own defect is planted in front of it, that dimension is decorative, and
that finding does not require a screenwriter to establish.

**v1 does not gate production promotion.** Gate 8 of the parent contract
(screenwriter sign-off) is unsatisfiable here. v1 outputs are advisory until the
professional slot defined in section 8 is filled.

## 2. Borrowed methodology and what was changed

| Source | Borrowed | Changed for this product |
| --- | --- | --- |
| HANNA (COLING 2022) | orthogonality requirement on criteria; the method of correlating automatic metrics against human criteria | criteria replaced: HANNA's Empathy/Surprise/Engagement/Complexity are reader-response constructs measured by crowd workers; this product needs craft judgement |
| StoryER (EMNLP 2022) | rating + ranking + **reasoning**; a score is invalid without a stated reason | reasoning must cite an artifact span, not free prose |
| CML-Bench | Dialogue Coherence / Character Consistency / Plot Reasonableness as mechanically checkable signals | demoted from scoring dimensions to automatic checks |
| Industry script coverage | separation of the diagnostic grid from a holistic three-tier verdict | verdict tiers renamed and bound to explicit floors rather than reader taste |

Only methods and structures are borrowed. No rubric text, beat-sheet wording,
or proprietary terminology from any framework is copied, and no corpus from
those projects enters the licensed reference library.

## 3. Admission gates (boolean, unscored)

Checked before any scoring. Any failure ends the case; no dimension scores are
produced.

- artifact JSON schema validation;
- episode count, minutes per episode, location and cast count within
  `constraints`;
- every deterministically detectable `required_elements` item present;
- `required_conditions` are evaluated by the judge and do not participate in
  admission;
- no `forbidden_elements` present;
- provenance manifest complete, every `source_id` licensed for `evaluation`;
- source overlap below the blocking threshold;
- safety and content policy pass.

Rights and safety are **not** scored dimensions in v1. The parent contract gives
safety a 2% weight, which allows trading compliance against craft. v1 removes
that trade by making it an admission condition.

## 4. Dimensions and aggregation

Ten scored dimensions in four pillars, each pillar weighted 25%.

| Pillar | Dimensions |
| --- | --- |
| `character_credibility` | 人的可信度、人物区分度、台词与潜台词 |
| `structure_causality` | 因果一致性、连续性 |
| `viewing_drive` | 情感推进、短剧节奏、题材兑现 |
| `originality_delivery` | 原创性、可制作性 |

Each dimension is scored 1–5. A pillar is the arithmetic mean of its dimensions.
The final score is the **geometric mean of the four pillars**:

```text
final = (P1 * P2 * P3 * P4) ^ (1/4)
```

Arithmetic weighted sum is retained only as a shadow metric, computed and stored
alongside the geometric mean so the two aggregations can be compared on real
data. It has no gating authority.

### 4.1 Why grouped, not flat

Flat per-dimension equal weighting would reduce 人的可信度 from 20% to 9% and
contradict the product thesis. Grouping keeps the human-credibility pillar at
25% while removing the compensation path that lets pacing and genre offset dead
characters.

The pillar assignment above is a prior, not a finding. Once 30 cases are scored,
compute the dimension correlation matrix; dimensions that correlate above the
threshold in the manifest belong in the same pillar, and the grouping is
revised. HANNA's orthogonality requirement is the reason this check exists.

### 4.2 Floors

Non-compensatory conditions, all required:

- every pillar `>= 3.0`;
- 人的可信度、原创性、因果一致性 each `>= 3.0`;
- reject if any dimension is scored at or below `1`.

### 4.3 Verdict

| Verdict | Condition |
| --- | --- |
| `reject` | any admission gate fails, or any floor fails |
| `consider` | all floors pass, final score below the manifest's pass threshold |
| `pass` | all floors pass and final score at or above the pass threshold |

## 5. Scoring subject in v1

Primary: **LLM judges**, at least two models from different families, plus a
third as tie-breaker when pillar means differ by more than the manifest's
disagreement threshold.

Secondary: **internal spot check** by non-professional reviewers on a stratified
sample (manifest-configured, default 20% of cases plus 100% of adversarial
pairs). Internal reviewers are not a substitute for screenwriters; their scores
exist to detect judge failure modes, not to establish ground truth.

### 5.1 Judge protocol

- the judge receives the artifact, the case constraints, and the rubric anchors;
- it never receives the split name, the incumbent output, seeded defect keys,
  or prior scores;
- every dimension score requires a reason **and a span reference**; a score
  without a locatable span is discarded and re-requested once, then recorded as
  invalid;
- three samples per artifact; the **median** is used, matching parent gate 9,
  which forbids selecting a best-of run;
- pointwise scoring and pairwise comparison are separate passes and never share
  a context.

### 5.2 Mandatory bias controls

LLM judges have known failure modes that would silently invalidate v1:

- **position bias** — every pairwise comparison is run in both orders and the
  result averaged; a pair that flips with order is recorded as undecided, not
  as a win;
- **length bias** — artifact length is recorded with every score, and the
  length/score correlation is reported per run; adversarial positives and
  negatives are length-matched;
- **self-preference** — a model does not judge artifacts generated by itself;
  the judge model set and the generator model set are disjoint;
- **verbosity of reasoning** — reasoning length is not rewarded and is capped.

## 6. Measuring the evaluator

This is the part v1 exists for. Every metric below compares a baseline artifact
against the same artifact with a known defect planted in it. None of them
requires a hard positive, which is why all of them are runnable now.

| Metric | Definition | v1 target |
| --- | --- | --- |
| `seeded_defect_detection` | share of seeded pairs where the degraded artifact scores lower than its own baseline | judge `>= 0.75` |
| `defect_localisation` | share of degraded artifacts where the judge's cited span overlaps the seeded defect span | `>= 0.50` |
| `perturbation_sensitivity` | share of minimal perturbations where the targeted dimension drops | `>= 0.70` |
| `perturbation_specificity` | two views: all non-target dimensions and dimensions outside the target pillar | cross-pillar `>= 0.70` |
| `self_consistency` | agreement across the three seeds of one judge | reported, no target in v1 |
| `inter_model_agreement` | Krippendorff's alpha across judge models | reported, no target in v1 |
| `spot_check_agreement` | agreement between judges and the internal sample | reported, no target in v1 |

`seeded_defect_detection` measures **within-pair direction**, not quality
ranking. It asks whether an evaluator scores a knowingly worsened version below
its own unmodified source. It says nothing about whether the source was good.

Report it together with `defect_localisation`. An evaluator that gets the
direction right but cannot say where the defect is has responded to a diffuse
signal — most likely length, tone, or fluency — rather than the planted defect.
Such an evaluator is unreliable for diagnosis even when the direction target is
met, and its dimension-level scores must not be used to attribute failure.

`perturbation_specificity` is always reported in two views:

- `specificity_all` checks every non-target dimension. It reveals whether the
  target pillar moves as a block and is diagnostic evidence for pillar review.
- `specificity_cross_pillar` excludes dimensions assigned to the target pillar.
  P2 uses this view for separability because within-pillar correlation is part
  of the current grouping hypothesis.

The legacy `specificity` and `min_specificity` fields alias the all-dimension
view for compatibility. A dialogue perturbation moving producibility still
reduces both views and remains evidence that the rubric is not separating skills.

### 6.1 Deferred to the professional version

| Metric | Why it cannot run in v1 |
| --- | --- |
| `pair_accuracy` | requires a hard positive; no available author can write one |
| genuine discrimination between two competent artifacts | same |
| absolute calibration of scores against craft judgement | requires a professional panel |

These are reserved names, not dropped requirements. The score record schema
already accommodates them, so adding them later is a manifest change rather than
a re-run.

Targets are manifest-owned and provisional. They are set from the first probe
run, not defended as principled values.

## 7. Adversarial set

Construction rules are in
[`STORY_EVAL_ADVERSARIAL.md`](STORY_EVAL_ADVERSARIAL.md). v1 builds only the
kinds that do not depend on a hard positive.

**Seeded pairs (built in v1).** A generated baseline artifact and the same
artifact degraded by one load-bearing defect. The baseline is the control, so no
claim is made that it is good — only that it is unmodified. The degradation must
be masked by a surface virtue the evaluator tends to reward, or the pair tests
nothing beyond obvious damage.

**Minimal perturbation pairs (built in v1).** A single targeted edit on the same
baseline, required to move exactly one dimension.

**Discrimination pairs (deferred).** A hard positive against a hard negative,
where hardness is a measured property of the pair. This is the version that
would support `pair_accuracy`, and it waits on an author who can write a
credible hard positive.

All members must pass every admission gate. A degraded artifact that trips an
automatic check is filtered before reaching the judge and measures nothing.

## 8. Reserved professional-review slot

v1 runs without screenwriters, but every structure they will need is built now,
so that engaging them requires no schema change and no re-run.

Reserved and populated-as-null in v1:

- `rater.rater_type` accepts `llm_judge`, `internal_spot_check`, and
  `professional`; all three share one rubric version and one anchor set;
- `blind_assignment_id` and `rater_blinded` on every score record, exercised in
  v1 by the internal spot check;
- per-artifact retention of **both** the judge score and any human score, so
  `judge_score - human_score` is computable per artifact — this is the sole
  input to harvesting real hard negatives later and cannot be reconstructed
  after the fact;
- `located_defect_spans` on every score record;
- `adjudication_required` flag when critical dimensions differ by 2 or more;
- Krippendorff's alpha computed over an arbitrary rater set, so the same code
  serves judges now and a panel later.

Deferred to the professional version, and explicitly absent in v1:

- calibration against the ten anchor artifacts of parent §7.3;
- the alpha `>= 0.67` requirement;
- pairwise preference as the release statistic;
- parent gate 8 sign-off.

## 9. Span addressing

`schemas/story-artifact-v1.json` is a reference wrapper; it carries a content
hash but no addressable interior. Reasoning citations, seeded defect keys, and
`defect_localisation` all require stable interior addresses, so v1 defines:

```text
span_ref := <artifact_type>/<node_path>
node_path := <segment>(/<segment>)*
segment  := <kind>-<index>        e.g. episode-3, scene-2, dialogue-7, beat-4
```

Content documents must emit a stable `node_id` per addressable leaf, and a
revision that supersedes an artifact must carry a node-level correspondence map
for unchanged nodes. Without that map, defect keys cannot survive a revision and
localisation metrics break across rounds.

## 10. Splits in v1

Parent splits are kept, with one temporary change: `holdout` is **not consumed**
in v1. It has no valid consumer without a professional panel, and spending it on
LLM judges would burn its blindness for no return.

v1 uses `dev` and `train` for construction and probing, `validation` for the
advisory gate, and `challenge` for the adversarial set. `holdout` stays sealed.

## 11. Layout

```text
eval/
  manifests/eval-v0.1.0.json
  rubrics/judge-v1.yaml
  cases/{dev,train,validation,holdout,challenge}/cases.jsonl
  adversarial/pairs.jsonl
  runs/<run_id>/
    config.json
    scores.jsonl
    evaluator_metrics.json
    summary.json
```

## 12. v1 delivery sequence

1. Freeze admission gates, rubric, and manifest thresholds. *(this document)*
2. Author seed cases in `dev`. *(started)*
3. Implement pillar aggregation, floors, and verdict in `story-eval`. *(started)*
4. Run the single-case degradation probe from the adversarial spec.
5. Run the seven-path masking probe; drop paths the judge already detects.
6. Build the adversarial set at the size the probe justifies.
7. Report evaluator metrics; only then score candidates for real.
8. Compute the dimension correlation matrix and revise pillar grouping.
9. Engage screenwriters and populate the reserved slot.

Steps 4 and 5 gate the size of step 6. Do not build the full adversarial set
before the probe result is known.

## 13. Boundary with online policy

Everything in this document is **offline**. The weighting that runs inside a
production job is a separate design system, specified in
[`ONLINE_POLICY_DESIGN.md`](ONLINE_POLICY_DESIGN.md) and implemented in
`crates/story-policy`.

The separation is enforced, not advisory:

- this rubric is never executed on the production path; doing so would make the
  instrument part of what it measures;
- online weights are a candidate change and are promoted through this
  evaluation, never tuned against production telemetry;
- neither manifest references the other's numbers — a duplicated constant is
  preferred over a shared one;
- the only shared object is the failure code vocabulary of parent §9.

Cost is the clearest illustration. Online must trade cost against quality
because it runs under a hard budget. This document must not, because a cost term
in the quality score would make a cheaper-but-worse candidate outscore a better
one, and the ability to distinguish "better" from "cheaper" is exactly what an
instrument is for.
