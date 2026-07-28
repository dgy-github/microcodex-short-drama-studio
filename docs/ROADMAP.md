# Roadmap

Version: `roadmap/v1.0.0` — see [`VERSIONS.md`](VERSIONS.md)
Scope: phase sequencing and exit criteria. `HANDOFF.md` records where the last
session stopped; this file records where the work is going. The two have
different lifetimes and are edited for different reasons.

No dates. Every phase ends on a stated condition, not on a calendar.

## The judgement this plan is built on

Instrument work has been over-weighted. There is one adversarial pair, one
scored case, and no generation pipeline, after several rounds of refinement on
that single pair. `STORY_MULTI_AGENT_DESIGN.md` §15 puts the evaluation pilot at
step 7, behind six runtime steps; building evaluation first was the right
inversion, because evaluation is the acceptance standard for everything else —
but the marginal value of a fifth round of probe polish is below that of a first
round of runtime.

So every instrument phase below carries an explicit stopping condition. The
failure mode this guards against is polishing the measuring device forever
without ever producing the thing being measured.

---

## P0 · Seal the current state

Commit the outstanding working tree. Archiving and contract metrics have landed;
this is a clean point. Left uncommitted, the tree and `HANDOFF.md` keep
diverging.

**Exit:** working tree clean.

---

## P1 · Calibrate the instrument until it can be read

**Exit: a readable, stable specificity figure from both judges on a narrowed
negative. Once obtained, stop — do not keep optimising this pair.**

1. ✅ Narrow the degradation from 12 dialogue lines to 1–2.
2. ✅ Report both `specificity_all` and `specificity_cross_pillar`; use the
   cross-pillar view for the P2 separability decision and retain the all view
   for pillar-collapse diagnosis.
3. Add an input fingerprint (artifact + rubric + judge config) to every judge
   result, so a stale result cannot be silently reused after an artifact edit.
4. Give `status` a specificity and stability threshold. The present version
   checks only sensitivity and order consistency, so a specificity of 0.11 still
   reports `measurable_gap`.
5. Re-run the 12 scoring calls and recompute `compute_evaluator_metrics.py`.
6. Implement the `inter_model_agreement` estimator.

Narrowing the degradation is listed first because it alone unlocks three
readings: specificity becomes interpretable, `defect_localisation` escapes being
constructively guaranteed, and `cited_span_precision` acquires a meaningful
denominator.

---

## P2 · Decision point

Not a task. P1 has three possible outcomes whose responses differ enough that
the branch is recorded now, before anyone is invested in a particular answer.

| P1 outcome | Meaning | Response |
| --- | --- | --- |
| specificity high | dimensions separable, the rubric holds | proceed to P3 |
| specificity still low with a minimal degradation | **dimensions are not separable; the rubric is decorative** | revise pillar grouping → MAJOR bump → every existing score becomes incomparable |
| judge stability still poor | neither Chinese-native family is reliable here | add a non-Chinese-native third family, or reduce reliance on LLM judges |

The middle row is the real risk: it means overturning the four-pillar design.
It is written down now so that, if it happens, sunk cost does not turn into a
decision to accept an unreadable figure.

---

## P2.5 · Sink the form-specific layer

Added 2026-07-27, after the product scope opened up: alongside scripted short
drama, the product may also cover knowledge/explainer content and real-creator
content. Those are not extra templates. They are different **content forms**,
and a form determines the artifact schema and the rubric, not just parameters
inside one.

The response is not to build three of everything, and not to defer. It is to
separate what is form-agnostic from what is form-specific, implement exactly one
form, and make adding a form a matter of supplying configuration.

Placed after P2 deliberately: P2 may already require revising the pillar
grouping, and "revise the pillars" and "stop hard-coding the pillars" are the
same edit. Merging them costs one MAJOR bump instead of two, and each MAJOR
invalidates existing scores and forces a baseline re-run.

### Scope decisions taken 2026-07-27

**One product, several forms — not several products.** The user switches form
inside the same application, which is what makes `content_form` a first-class
job-contract field rather than a build-time choice. Item 3 below moved from
"probably useful" to "required" on the strength of this.

What that splits:

| Shared across forms | One per form |
| --- | --- |
| desktop shell, job-contract envelope, event/SSE protocol | artifact schema |
| storage, rights, budget, cost accounting | rubric — dimensions and anchors |
| online policy: `D1` selection, `hard_rules`, tie-breaks | case set |
| aggregation arithmetic (item 1, done) | the specific admission checks |

**Level 2 uses the creator-console publishing category.** Of the three Douyin
taxonomies available — publishing category, internal recommendation labels,
industry-report genres — the publishing category is the creator-side one, so it
matches what this product's users already select when they publish and needs no
translation. Recommendation labels are internal, unpublished and algorithm-
dependent. Industry-report genres disagree between sources: an earlier market
check found the ad-spend, Douyin-native and filing taxonomies not directly
comparable.

**Switching form starts a new project.** An existing project keeps the form it
was created with. This deliberately rules out cross-form artifact translation —
turning a drama draft into an explainer draft — which would be an entirely new
mechanism with no acceptance standard behind it.

The payoff is that `content_form` becomes immutable for the life of a project,
so a job, its artifacts, its rubric and its scores can never disagree about
which form they belong to. Enforce it at job creation: the field is set once and
rejected on update.

### Why Douyin's categories are not the top-level axis

Douyin's categories are distribution and recommendation labels. They are
orthogonal to production form: one 美食 label covers an explainer about
ingredients, a real-creator shop visit, and a scripted story set in a small
restaurant. Three different artifact structures under one label, so the label
cannot determine a schema. Content form is the Level 1 axis; platform
categories belong at Level 2, or as market-targeting metadata.

Level 1 is decided by structure: does the artifact have character arcs? scene
dialogue? who is in front of the camera?

### Status

| # | Item | State |
| --- | --- | --- |
| 1 | Pillars and critical dimensions read from the manifest instead of being struct fields in `story-eval` | **done** |
| 2 | Span addressing extracted from `story-package/v1` into a form-agnostic convention; revision correspondence remains in P6 | **done** |
| 3 | `content_form` promoted to a first-class field on the job contract, selecting which (schema, rubric, case set) triple to load | open |

Item 1 was run first as a falsification test of this whole phase: if separating
the layers had been painful, the form-agnostic layer would have been thinner
than claimed and the plan would need re-estimating. It was not painful. The
change touched one crate; `story-policy`, `story-core`, `story-runtime` and the
Python tooling were untouched, the nine existing tests kept passing, and two new
tests pin the claim down — one loads the real repository manifest, the other
aggregates a hypothetical three-pillar explainer rubric with no code change.

**Exit:** adding a content form requires an artifact schema, a rubric and a case
set, and no edit to aggregation, gating or verdict logic.

### Explicitly not in scope

No explainer or real-creator rubric, no cases, no new templates. The eval
surface multiplies by form count, and no single form has yet been carried end to
end through the runtime. With a sub-1% break-out rate in this market, the lever
is quality within a form, not the number of forms.

Scripted drama stays first for two reasons. It is the only form with assets — 30
cases, 10 baselines, a rubric, a probe. More importantly it is the **hardest**
form: human credibility, subtext and continuity all matter there and mostly do
not in explainer content. Building the hard form first lets the machinery adapt
downward; building the easy one first would leave the hard requirements
undiscovered until they force a rewrite.

---

## P3 · Freeze the evaluation set, then build the runtime

Sequenced, not parallel. Every runtime step needs the evaluation set to accept
it; an unfrozen set is an absent acceptance standard, so runtime work done first
would have to be re-judged afterwards.

### P3a · Evaluation set to frozen

- Score the remaining 9 baselines → 30 scored cases.
- Compute the dimension correlation matrix → pillar grouping review. The
  manifest's `pillar_grouping_review` has been waiting on exactly this.
- Run the internal spot check, unlocking `spot_check_agreement`.
- **Freeze `eval-v0.1.0` and `judge-v1`.**

**Exit:** manifest and rubric frozen; a MAJOR bump is required to change them.

### P3b · Runtime, per `STORY_MULTI_AGENT_DESIGN.md` §15

- Sidecar lifecycle and authenticated localhost transport.
- Async command acceptance, EventLog replay, SSE resume, heartbeat,
  deduplication, backpressure.
- One fixed `ExecutionOrder`; free-form decomposition stays off.
- Register the genre, three-architect, episode, scene and three reviewer lanes.
- Licensed retrieval manifest and provenance checks.

**Exit:** one job runs end to end and produces a `story-package/v1` through the
DAG, scored by the frozen evaluation set — the multi-agent arm of the
model-only-versus-multi-agent comparison the parent contract requires. The
model-only arm already exists as the 10 archived baselines.

### Deferred within P3

**Stage 1 seven-path masking probe.** It serves mass production of the
adversarial set; the adversarial set serves discrimination measurement; and
discrimination is not measurable in v1 at all, for want of a hard positive. It
ranks below reaching 30 scored cases and freezing.

---

## P4 · Make story decisions observable and correctable

P4 remains downstream of P3b. One storage boundary is implemented early because
the first DAG run must not discard evidence that cannot be reconstructed.

| Item | State | Gate |
| --- | --- | --- |
| Retain every `t06` candidate, including losers and online scores | **done: interface + validation** | concrete persistence lands with P3b storage |
| Give `story-policy::Defect` a stable span so `D3` can target a revision | **done** | form-agnostic `ArtifactSpanRef` |
| Design and implement `D5` model routing | open | provider inventory and frozen eval |
| Design and implement `D6` reserve-writer takeover | open | runtime failure taxonomy and budget events |
| Compute `proxy_fidelity`, then replace placeholder online weights | blocked | first retained production candidate set |
| Engage screenwriters for hard positives and genuine discrimination | external | reviewer protocol and procurement |

Knowledge/explainer, real-creator, video download and video material extraction
are outside the current story-writing release. They are not P4 tasks.

**Exit:** every online selection and revision decision is replayable; rejected
candidates remain available for offline comparison; D3, D5 and D6 have bounded,
deterministic fallbacks; online weights have measured proxy fidelity.

---

## P5 · Usable story-authoring desktop

Build the smallest desktop surface that can operate the P3/P4 runtime without a
developer console:

- create a story project with immutable `content_form=scripted_drama`;
- capture premise, genre, episode/length constraints and budget;
- start/cancel a job and reconnect to its event stream;
- show task progress, approvals, failures and budget state;
- inspect versioned architecture, episode, scene and final story artifacts;
- **encrypted storage for provider credentials, pulled forward from P9.**

The Svelte/Tauri shell calls Rust only. It neither holds provider credentials
nor connects directly to the Campaign sidecar.

### Why credential encryption moved here — settled 2026-07-27

**Users supply their own provider keys.** This is not an open question; the
design already answers it. P10 requires "first-run provider configuration and
health diagnostics" and exits on "a clean machine can install, configure,
complete and export a story job". `ARCHITECTURE.md` assigns provider keys to the
Rust product. There is no billing, subscription, proxy or hosted layer anywhere
in the design — this is an installed Windows application with no server, so the
keys can only be the user's own.

The only real question was whether that starts at P5 or at P10, and encryption
belongs at P5 either way, on cost asymmetry:

- **No credential storage exists yet.** `story-provider` is a trait; the tooling
  reads environment variables. Writing the store encrypted costs almost nothing
  extra — it is the same code path. Retrofitting means migrating already-stored
  plaintext, supporting both formats, and handling upgrade paths.
- **Plaintext keys leak through ordinary workflow, not through releases.**
  During this project's own development a key reached `.env` and came close to
  reaching a tracked file. Between P5 and P8 the keys sit on disk regardless of
  who is using the application.

If P5 turns out to be internal-only, nothing is lost: the code had to be written
anyway.

P9 keeps rotation, audit and the wider security review. Only at-rest encryption
moves.

**Exit:** a user can create, run, interrupt, resume and inspect one complete
story-writing job from the desktop, with any credentials they supplied held
encrypted at rest.

---

## P6 · Directed revision and approval

Turn generated output into an editable writing workflow:

- populate revision correspondence. The schema already exists: `story-package/v1`
  declares `node_correspondence` and binds it to `supersedes` via
  `dependentRequired`, and form-agnostic span addressing landed in P2.5. What is
  missing is the implementation that fills the map on revision, not the design;
- defect-to-span navigation;
- bounded D3/D4 revision rounds with explicit approval;
- immutable revision history, comparison and rollback-by-new-revision;
- export of a validated `story-package/v1`.

**Exit:** a user can revise a cited story location, approve the result and export
an auditable package without mutating history.

---

## P7 · Professional quality gate — **starts in parallel from P3a, not after P6**

Fill the professional-review slot reserved by `STORY_EVAL_V1.md`:

- blind screenwriter calibration on anchor artifacts;
- hard-positive and discrimination-pair construction;
- `pair_accuracy`, professional agreement and adjudication;
- sealed holdout execution;
- promotion rules for model, prompt, graph, retriever and skill candidates.

LLM judges may continue filtering but cannot promote a candidate alone.

### Why this is not a sequential P7

Its real dependencies are a frozen rubric (P3a) and artifacts to judge (P3b).
It needs neither the desktop shell nor the revision workflow. What it does need
is **people**, and recruiting, contracting and calibrating professional
screenwriters carries external lead time that no amount of engineering removes —
P4 already marks this item `external`.

Run sequentially and it produces the failure this whole evaluation design exists
to prevent: a feature-complete product that no qualified reader has ever
assessed, promoted entirely on LLM judgement. The number keeps its position for
readability, but **procurement starts as soon as P3a freezes**, and the phases
after it must not be read as blocking it.

**Exit:** one candidate-versus-incumbent release decision is made from hidden
human review with complete provenance.

---

## P8 · Story domain breadth

Expand within story writing, not into video processing:

- versioned genre packs and constraint profiles;
- long/short episode-count variants that share the same story contracts;
- genre-specific architect/reviewer configuration;
- licensed retrieval collections with per-source provenance;
- regression cases for every promoted genre pack.

Adding a genre must not change runtime, storage or evaluation arithmetic.

### Keeping the evaluation set alive

Regression cases cover *additions*. Two standing obligations cover *decay*, and
both were previously recorded as required and then lost when the phases were
written:

- `STORY_EVAL_DESIGN.md` §4 requires the `challenge` split to refresh
  quarterly from production failures;
- `STORY_EVAL_ADVERSARIAL.md` §8 leaves adversarial retirement rules explicitly
  undefined and tracked as open.

A fixed adversarial set is Goodharted within a few iterations — our own
conclusion, in our own document. Skills and prompts learn to avoid the specific
samples rather than the class of defect, and the metrics keep looking healthy
while the capability they measure erodes.

This is recurring work, not a one-off item: define the retirement rule, then run
the refresh on a cadence. It has no exit condition because it does not end.

**Exit:** a second scripted-drama genre pack is promoted through the same hidden
gate without core-code changes, and a defined retirement rule governs the
adversarial set.

---

## P9 · Production reliability and governance

- credential rotation and audit (at-rest encryption moved forward to P5);
- crash recovery, event-log repair, backup and schema migration;
- concurrency, budget, timeout and provider-degradation tests;
- structured diagnostics with secret and chain-of-thought redaction;
- dependency/license inventory, security review and incident runbook.

**Exit:** fault-injection, migration, backup/restore, security and sustained-run
checks pass with no loss of durable story or approval state.

---

## P10 · Stable story-studio release

- signed Windows packaging and reproducible build evidence;
- first-run provider configuration and health diagnostics;
- accessibility, localization and operator documentation;
- upgrade/rollback compatibility policy;
- stable contract/version declarations and release notes.

The stable release remains a story-writing product. Video download, video
material extraction and automatic publishing require a separate future scope
decision.

**Exit:** a clean machine can install, configure, complete and export a story
job, upgrade safely, and reproduce the release evidence.

---

## Deliberately downgraded

Part of planning is refusing work that is not worth its cost now.

| Item | Decision |
| --- | --- |
| `docs/eval-governance.html` | Stop hand-maintaining it. It is a derived view of the manifest, so manual upkeep guarantees recurring drift — the current `pair_accuracy` / `seeded_defect_detection` inconsistency is the second instance. Regenerate it from the manifest instead, or delete it. |
| Similarity search of the 30 premises against existing screen works | Defer until just before freezing. Nothing is in production, so the cost is real and the payoff is not yet. |
| MinHash / embedding family checking | Defer to 120 cases. At 30, reading them is sufficient and has already caught two collisions that label inspection missed. |

---

## Dependency summary

```text
P0 seal
  └─ P1 calibrate ──► P2 decide
                        ├─ rubric holds ──────────┐
                        ├─ rubric decorative ──► revise pillars (MAJOR) ──► P1
                        └─ judges unreliable ──► add third family ──► P1
                                                  │
                        P2.5 sink the form layer ◄┘
                          └─ P3a freeze ─┬─► P3b runtime ──► P4 decisions
                                         │     └─► P5 desktop ──► P6 revision
                                         │           └─► P8 breadth ──► P9 ──► P10
                                         │
                                         └─► P7 professional gate ─────────┘
                                             (parallel; external lead time,
                                              start procurement at P3a)
```

P1 is the only gate every path passes through. P3b is the first phase that
produces a story the product could ship. P5 makes it usable without a developer
console. P10 is the stable release.

**P7 is drawn as a parallel branch on purpose.** It is the only phase gated on
recruiting people rather than writing code, and it is the only one that can
promote a candidate. Sequencing it behind the desktop and the revision workflow
would let the product reach feature completeness having never been read by
anyone qualified to judge it.
