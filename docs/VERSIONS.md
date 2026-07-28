# Design Document and Artifact Versions

Registry of every versioned design document and the data or configuration it
governs. A document version answers one question: **are two scores comparable?**

## 1. Registry

| Document | Version | Status | Governs |
| --- | --- | --- | --- |
| [`STORY_EVAL_DESIGN.md`](STORY_EVAL_DESIGN.md) | `story-eval-target/v1.0.0` | target contract | the professional-panel end state; not runnable yet |
| [`STORY_EVAL_V1.md`](STORY_EVAL_V1.md) | `story-eval-offline/v1.0.0` | active, advisory | `eval/manifests/eval-v0.1.0.json`, `eval/rubrics/judge-v1.yaml`, `crates/story-eval` |
| [`STORY_EVAL_ADVERSARIAL.md`](STORY_EVAL_ADVERSARIAL.md) | `story-eval-adversarial/v1.0.0` | active | `eval/adversarial/pairs.jsonl`, `schemas/eval-adversarial-pair-v1.json` |
| [`ONLINE_POLICY_DESIGN.md`](ONLINE_POLICY_DESIGN.md) | `online-policy-design/v1.0.0` | active, unpromoted | `schemas/online-policy-v1.json`, `crates/story-policy` |
| [`STORY_MULTI_AGENT_DESIGN.md`](STORY_MULTI_AGENT_DESIGN.md) | `story-multi-agent/v1.0.0` | active | `schemas/story-job-v1.json`, `schemas/story-agent-event-v1.json`, `crates/story-runtime` |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | `architecture/v1.0.0` | active | crate ownership and process boundaries |
| [`ROADMAP.md`](ROADMAP.md) | `roadmap/v1.0.0` | active | phase sequencing and exit criteria; no artifacts |

### Data and configuration versions

| Artifact | Version | Bound to |
| --- | --- | --- |
| `eval/manifests/eval-v0.1.0.json` | `eval-v0.1.0` | `story-eval-offline/v1.0.0` |
| `eval/rubrics/judge-v1.yaml` | `judge-v1` | `story-eval-offline/v1.0.0` |
| `eval/cases/<split>/cases.jsonl` | frozen with the manifest | `schemas/eval-case-v1.json` |
| `schemas/story-package-v1.json` | `story-package/v1` | `story-eval-offline/v1.0.0` §9 span addressing |
| `policy/online-policy-v*.json` | `online-policy/v1` schema | `online-policy-design/v1.0.0` |

Document versions and data versions move independently. Editing a threshold
bumps the manifest, not the design document. Changing how thresholds are
combined bumps both.

## 2. Bump rules

Semantic versioning, with comparability as the deciding criterion.

| Bump | Trigger | Effect on stored scores |
| --- | --- | --- |
| MAJOR | decision semantics change — aggregation formula, floors, admission gates, verdict tiers, gate conditions | **not comparable**; the baseline must be re-run before any candidate is judged against it |
| MINOR | additive and backward compatible — a new dimension, a new evaluator metric, a new optional field | comparable on the pre-existing dimensions only |
| PATCH | wording, anchor clarification, typo, added rationale | fully comparable |

The test for MAJOR is mechanical: if replaying an old artifact through the new
version could change its verdict, it is MAJOR. Two examples already on file —
switching aggregation from weighted sum to geometric mean, and moving safety out
of the scored dimensions into admission — would both have been MAJOR.

Every run record stores the document version, the manifest version, and the
rubric version it executed under. A run whose versions differ from the incumbent
is not silently compared.

## 3. Cross-system versioning

Offline evaluation and online policy version independently, and the dependency
runs one way:

- an `online-policy-v*.json` instance records `promoted_by_eval_run`, plus the
  `eval_version` in force at promotion time;
- promoting a policy config **never** bumps an offline document version — the
  instrument does not change because something it measured changed;
- bumping `story-eval-offline` MAJOR invalidates every existing
  `promoted_by_eval_run`, because the promotion evidence was produced by a
  different instrument. Policy configs revert to unpromoted and must re-run.

That last rule is the versioning expression of the one-way dependency in
`ONLINE_POLICY_DESIGN.md` §2. It is the only coupling between the two systems
and it is deliberately asymmetric.

## 4. Current freeze state

| Item | State |
| --- | --- |
| `eval-v0.1.0` | **not frozen** — thresholds are provisional, set from probe runs rather than principle |
| `judge-v1` | not frozen — anchors untested against real artifacts |
| dev case set | 30 cases, pending the fixes in `HANDOFF.md` |
| `online-policy` instance | none exists; weights in `crates/story-policy` are placeholders |
| `story-eval-offline/v1.0.0` | active but advisory; does not gate production promotion |

Nothing here is frozen yet. `eval-v0.1.0` may be frozen only after the pillar
grouping review (30 scored cases, dimension correlation matrix) and the stage 0
adversarial probe, because both can still change the aggregation — that is, both
can still force a MAJOR bump.
