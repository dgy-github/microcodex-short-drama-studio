# Online Policy Design

Version: `online-policy-design/v1.0.0` — see [`VERSIONS.md`](VERSIONS.md)
Status: active, unpromoted. No policy instance exists; weights are placeholders.
Counterpart system: [`STORY_EVAL_V1.md`](STORY_EVAL_V1.md).
Governs: `schemas/online-policy-v1.json`, `crates/story-policy`.

Scope: the weighting system that makes decisions **inside a production run**.

This is a separate design system from offline evaluation
([`STORY_EVAL_V1.md`](STORY_EVAL_V1.md)). The two are related by a one-way
dependency and a shared vocabulary, and by nothing else. Merging them is the
single most likely way to destroy both.

## 1. Why they cannot be one system

They answer different questions under incompatible constraints.

| | Offline evaluation | Online policy |
| --- | --- | --- |
| Question | is this **version** good enough to ship | which **option** do I take right now |
| Decision type | absolute thresholding | relative ranking |
| Unit | a frozen case set | one job |
| Ground truth | human judgement, eventually | none exists at runtime |
| Sample size | many cases, 3 seeds each | n = 1 |
| Latency budget | days | inside `deadline_seconds` |
| Cost budget | spend what it takes | inside `max_tokens` / `max_cny_fen` |
| Cost in the score | excluded on purpose | must be traded explicitly |
| Runs | per candidate change | every job |
| Failure mode | measures the wrong thing | picks the wrong option |
| Is it an instrument | yes | no, it is a strategy |

Four of these are load-bearing:

**Ranking is not thresholding.** Choosing the best of three architectures needs
only ordinal information and no calibration. Deciding whether a story ships
needs a calibrated absolute scale. A rubric built for the second is more
expensive than the first requires, and a proxy built for the first is not
trustworthy for the second.

**Online has no ground truth.** Offline eventually gets a human. At runtime
there is no reference answer, no reviewer panel, and no second chance. Every
online signal is necessarily a proxy.

**n = 1 kills variance tolerance.** Offline absorbs judge variance with three
seeds and a median across many cases. Online decides once. A signal with high
sample variance is unusable online even when it is excellent offline.

**Cost must be scored online and must not be scored offline.** Online runs under
a hard budget and must trade quality against spend. Offline must not: if cost
entered the quality score, a cheaper-but-worse candidate could outscore a
better one and you would lose the ability to tell whether a change improved
stories or merely made them cheaper. Offline handles cost as a separate gate,
never as a score term.

## 2. The one-way dependency

```text
online policy config  ──treated as a candidate change──►  offline evaluation
                                                                 │
                      ◄──────── promoted / rejected ─────────────┘

offline evaluation  ──never reads──►  production telemetry as criteria
```

Two rules, both absolute:

1. **Online weights are a versioned artifact promoted through offline
   evaluation.** Changing a selection weight is a candidate change exactly like
   changing a prompt. It gets a version, a run, and a gate.
2. **Offline criteria never adapt to online outcomes.** Production telemetry may
   become offline *input* — harvested hard negatives, new challenge cases — but
   never offline *criteria*. Tuning the instrument to make production look good
   destroys the instrument. This is the exact mechanism of Goodhart's law and
   the reason the two systems are physically separate crates.

## 3. What they do share: the failure vocabulary

The only shared object is the failure code set: `HUMAN_GENERIC`,
`MOTIVE_EXPLICIT`, `PLOT_CONVENIENCE`, `VOICE_COLLAPSE`, `EMOTION_UNEARNED`,
`HOOK_FAKE`, `TROPE_STACK`, `EXPOSITION`, `CONTINUITY`, `UNSHOOTABLE`,
`SOURCE_OVERLAP`, `POLICY`.

Both systems speak this vocabulary. Neither shares its scoring:

- offline maps codes to **rubric dimensions** and scores 1–5;
- online maps codes to **severity weights** and counts.

A shared vocabulary keeps the two comparable without coupling them. When offline
analysis shows that `MOTIVE_EXPLICIT` is systematically under-penalised in
production, that finding is expressible as an online weight change, and that
change is then evaluated offline. The loop closes through the vocabulary, not
through a shared score.

## 4. Online decision points

Every place in the DAG where the runtime picks between options. These are the
only places online weights apply.

| ID | Decision | Input available at that moment | Type |
| --- | --- | --- | --- |
| `D1` | select among architectures A / B / C (`t06`) | three structured proposals, retrieval manifest | ranking |
| `D2` | accept or reject a review (`t16`) | four reviewer reports | fail-closed threshold |
| `D3` | which defects to repair (`t15`) | defect list, remaining rounds, remaining budget | ranking under constraint |
| `D4` | revise again or return `input_required` | round count, unresolved critical defects | threshold |
| `D5` | model preset per task | task kind, remaining budget, elapsed time | constrained choice |
| `D6` | replace a worker with `reserve-writer` | failure kind, retry count | threshold |

`D2` is not a weighting decision and must never become one. The final reviewer is
fail-closed: missing, unparsable, timed-out, or exception-producing reviews are
rejections. No weight may convert a critical defect into an acceptable one.

## 5. Online currency: weighted defects, not rubric scores

The obvious mistake is to run the ten-dimension rubric on each candidate at
`D1`. That would cost roughly one full offline scoring pass per architecture,
three times per run, inside a 600-second deadline. It is unaffordable and
unnecessary.

The DAG already produces what online needs: `t11`–`t14` emit structured review
artifacts containing located defects with codes and severities. Online policy
weights **those**, and issues no extra scoring call.

```text
penalty(candidate) = Σ over defects [ severity_factor × code_weight ]
selection_score    = base_quality_signal − penalty − cost_term
```

Where:

- `base_quality_signal` is deliberately weak — candidate completeness and
  constraint satisfaction, not a taste judgement;
- `code_weight` is the tunable part, keyed on the shared vocabulary;
- `cost_term` reflects projected token and wall-clock consumption to finish that
  lane, not spend already incurred (sunk cost must not influence the choice);
- `severity_factor` for `critical` is not a large number — critical defects are
  handled by a separate hard rule, because a large-but-finite weight is still
  purchasable by a sufficiently attractive candidate.

That last point is the same non-compensatory principle as the offline pillar
floors, arriving from a different direction: anything that must never be traded
gets a rule, not a weight.

## 6. Constraints specific to online

**Determinism under retry.** The same inputs must produce the same decision. A
selection that flips between retries makes runs irreproducible and makes the
event log useless for diagnosis. Ties break on a declared deterministic key, not
on iteration order.

**No self-report.** A candidate's own confidence is not an input. Agents that
score their own work introduce an incentive the runtime cannot audit.

**Bounded evidence.** Decisions read structured artifacts only. Chain-of-thought
is not persisted and is therefore not available and must not be relied upon.

**Budget is a constraint, not an objective.** The policy minimises nothing; it
picks the best option that fits. A policy that optimises for low cost will
converge on short, cheap, bad stories, and offline evaluation will only notice
one release later.

## 7. The bridge: proxy fidelity

Two systems drift unless something measures the gap. The linking metric:

```text
proxy_fidelity = rank correlation between
                 the online selection order over N candidates
                 and the offline rubric order over the same N candidates
```

Computed offline, on stored production candidates. It answers the only question
that matters about the online policy: **does the cheap fast proxy pick what the
expensive calibrated instrument would have picked?**

Interpretation:

- high fidelity — the proxy is doing its job; weight changes can be trusted;
- low fidelity with correct top-1 — acceptable; ranking the tail is not needed;
- low fidelity at top-1 — the online policy is actively selecting worse
  candidates, and every run is being degraded by the thing meant to improve it.

This metric requires storing **all** candidates including rejected ones, with
their online scores. Discarding losers at `t06` makes proxy fidelity permanently
uncomputable. This is a storage requirement that must land before the DAG runs.

## 8. Anti-patterns

- **Reusing the offline rubric online.** Unaffordable, and it silently makes the
  instrument part of the production path, after which it can no longer measure
  that path independently.
- **Tuning online weights against production telemetry.** Optimises the proxy
  against itself. Weights change only through an offline gate.
- **Giving critical defects a large weight instead of a rule.** A large weight
  is a high price, and prices get paid.
- **Letting online cost pressure reach offline scores.** Destroys the ability to
  separate "better" from "cheaper".
- **Discarding rejected candidates.** Cheap at runtime, permanently destroys
  proxy fidelity.
- **One config file for both.** Physical separation is the enforcement
  mechanism; `story-eval` and `story-policy` are separate crates for this
  reason.

## 9. Configuration ownership

| Concern | Owner | Versioned with |
| --- | --- | --- |
| dimension weights, pillar grouping, floors, pass threshold | `eval/manifests/eval-v*.json` | the evaluation set |
| defect code weights, cost terms, tie-break keys, retry thresholds | `policy/online-policy-v*.json` | the product release |
| failure code vocabulary | `docs/STORY_EVAL_DESIGN.md` §9 | shared, changes to it affect both |

Neither manifest may reference the other's numbers. A duplicated constant is
preferable to a shared one, because a shared one creates the coupling this
document exists to prevent.

## 10. Delivery order

1. Freeze the decision point list and the shared vocabulary. *(this document)*
2. Define `online-policy/v1` config schema. *(done)*
3. Implement defect-weighted selection with deterministic tie-breaking in
   `story-policy`. *(started — `D1` and `D3` only)*
4. Add the storage requirement: retain rejected candidates with online scores.
5. Wire `D1` into the DAG once a DAG exists.
6. Compute proxy fidelity on the first stored candidate set.
7. Only then tune any weight.

Step 7 is last on purpose. Tuning before fidelity is measurable is guessing with
extra steps.
