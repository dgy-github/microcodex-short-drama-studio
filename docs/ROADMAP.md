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
3. ✅ Add an input fingerprint (artifact + rubric + judge config) to every judge
   result, so a stale result cannot be silently reused after an artifact edit.
4. ✅ Give `status` a specificity and stability threshold. The present version
   checks only sensitivity and order consistency, so a specificity of 0.11 still
   reports `measurable_gap`.
5. ✅ Re-run the 12 scoring calls and recompute `compute_evaluator_metrics.py`.
6. ✅ Implement the `inter_model_agreement` estimator.

### Latest calibrated reading — 2026-07-27

All saved results share one verified base-input fingerprint. The supplemental
Codex result also carries its exact judge-configuration fingerprint. Qwen
reached cross-pillar specificity `0.7143` and self-consistency `0.925`, but
flipped with artifact order. GLM reached `0.1429/0.475` and also flipped.
The local Codex `gpt-5.4` judge reached `0.4286/0.675`, detected the target
degradation, and was order-consistent. Krippendorff interval alpha across all
three judges rose from `0.3312` to `0.5175`; strict seeded-pair detection
remains `0.0` because Qwen scored the degraded artifact lower in only half of
its six observations.

The engineering checklist is complete but the P1 exit condition is not. P2
therefore remains on the `judge stability still poor` branch. The requested
non-Chinese-native third family has now been measured and did not clear the
stability/specificity thresholds. The remaining high-value evidence is the
internal blind human spot check.

### Deferral decision — 2026-07-28

P1 remains unresolved, but its human check is deferred until the first
end-to-end story package exists. P2.5 and an advisory P3b prototype are
unlocked under [`ADR-0002`](adr/ADR-0002-advisory-runtime-before-human-calibration.md).
This is not a pass:

- `eval-v0.1.0` and `judge-v1` cannot freeze;
- no model, prompt, graph, retriever, policy or skill can be promoted;
- prototype output is `advisory/non-promotable`;
- the first end-to-end artifact feeds the blind human check;
- the run must be repeated after P3a freezes the evaluation contract.

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
| 3 | `content_form` promoted to a first-class field on the job contract, selecting which (schema, rubric, case set) triple to load | **done**; `content-form-registry/v1` binds the real scripted-drama assets in `story-runtime` |

Item 1 was run first as a falsification test of this whole phase: if separating
the layers had been painful, the form-agnostic layer would have been thinner
than claimed and the plan would need re-estimating. It was not painful. The
change touched one crate; `story-policy`, `story-core`, `story-runtime` and the
Python tooling were untouched, the nine existing tests kept passing, and two new
tests pin the claim down — one loads the real repository manifest, the other
aggregates a hypothetical three-pillar explainer rubric with no code change.

**Exit:** adding a content form requires an artifact schema, a rubric and a case
set, and no edit to aggregation, gating or verdict logic.

**Exit reached:** the registry is data-driven, the runtime rejects duplicate or
unsafe bindings, and the checked-in scripted-drama entry resolves only existing
assets. Adding a later form still requires extending the closed product
`ContentForm` contract, but does not require changing aggregation, gating,
verdict or runtime lookup logic.

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

## P3 · Build an advisory runtime, then freeze and re-judge

The original plan required P3a before P3b. ADR-0002 accepts a bounded reversal:
build one advisory runtime path first to produce a real artifact, use that
artifact in the deferred human check, freeze the evaluation contract, then
repeat the runtime run. Prototype correctness can be tested before story
quality is promotable; the two claims must not be conflated.

### P3a · Evaluation set to frozen

- Score the remaining 9 baselines → 30 scored cases.
- Compute the dimension correlation matrix → pillar grouping review. The
  manifest's `pillar_grouping_review` has been waiting on exactly this.
- Run the internal spot check, unlocking `spot_check_agreement`.
- **Freeze `eval-v0.1.0` and `judge-v1`.**

**Exit:** manifest and rubric frozen; a MAJOR bump is required to change them.
This remains blocked on the deferred human check and blocks every promotion and
release claim, but no longer blocks the advisory P3b prototype.

### P3b · Runtime, per `STORY_MULTI_AGENT_DESIGN.md` §15

- Sidecar lifecycle and authenticated localhost transport.
- Async command acceptance, EventLog replay, SSE resume, heartbeat,
  deduplication, backpressure.
- One fixed `ExecutionOrder`; free-form decomposition stays off.
- Register the genre, three-architect, episode, scene and three reviewer lanes.
- Licensed retrieval manifest and provenance checks.

**Advisory prototype status:** the Rust fail-closed lifecycle, fixed 17-task
`ExecutionOrder`, real Python process supervision, authenticated localhost
host, idempotent `StartRun`, EventLog replay, typed Rust provider capability,
agent registration, structured reviews and artifact packaging are
integration-tested. Run `run_0148aa190ce842c8b103d3885a68dfcb` completed
17/17 tasks and produced a schema-valid six-episode `story-package/v1` with five
review records. Generation used DeepSeek `deepseek-v4-pro` through the standard
Chat Completions endpoint; review used Qwen `qwen3-vl-plus`. Provider-family
separation is now exercised, while story quality remains unverified.
Resume/Cancel and live SSE backpressure remain.

**Prototype exit reached:** one job ran end to end and produced an
`advisory/non-promotable` `story-package/v1` through the DAG. The artifact is
now the input to the deferred blind human check.

**Release-level exit:** after P3a freezes the evaluation set, repeat the job and
score it with the frozen contract — the multi-agent arm of the
model-only-versus-multi-agent comparison. The model-only arm already exists as
the 10 archived baselines.

### Deferred within P3

**Stage 1 seven-path masking probe — authored 2026-08-27, measurement pending.**
The six new degradations (hook-fake, false-payoff, emotion-unearned,
voice-collapse, plot-convenience, trope-stack) live under
`eval/adversarial/stage1/` on the shared comedy_002 base; MOTIVE_EXPLICIT
remains the stage-0 narrow pair. Measurement through `run_stage0_probe.py` is
blocked on judge credentials (see `HANDOFF.md`), not on tooling. The §9 branch
decision (15-40 pairs vs perturbation) still follows the reading. Its original
deferral rationale stands recorded: it serves mass production of the
adversarial set; the adversarial set serves discrimination measurement; and
discrimination is not measurable in v1 at all, for want of a hard positive.

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

- create a story project with immutable `content_form=scripted_short_drama`
  (**done in the Rust contract, Tauri command and desktop form**);
- capture premise, genre, episode/length constraints and budget
  (**done in `StoryJob` and the desktop binding**);
- start/cancel a job and reconnect to its event stream;
- show task progress, approvals, failures and budget state;
- inspect versioned architecture, episode, scene and final story artifacts;
- **encrypted storage for provider credentials, pulled forward from P9
  (done: Rust `story-provider` boundary + verified Windows Credential Manager
  backend + desktop configuration UI).**

The Svelte/Tauri shell calls Rust only. It neither holds provider credentials
nor connects directly to the Campaign sidecar.

**First desktop vertical slice reached:** the Tauri 2.8 + Svelte 5 shell now
validates `story-job/v1`, configures DeepSeek/Qwen credentials through Windows
Credential Manager, and browses completed advisory workflow artifacts through
typed Rust IPC. The second slice adds Rust-owned DeepSeek/Qwen runtime launch,
Start, idempotent Cancel, `Last-Event-ID` event recovery, task/review/approval/
error/token-budget projection, and validated artifact persistence. Process-level
Start/replay/Cancel integration passed. A deterministic desktop E2E now rejects
a duplicate Start, runs the real Python sidecar through the authenticated
capability boundary, completes 17 tasks and 5 reviews, performs both package
validations, and persists the advisory artifact. The paid provider route remains
ignored until rotated credentials are configured through Credential Manager.
Provider endpoint and model settings are editable and retained by Rust. Health
checks, the story runtime and automatic evaluation resolve the same validated
route record, so an endpoint change cannot leave one path on a different
hard-coded URL.
The desktop also exposes a Rust-owned evaluation center: the 30-case offline
catalog (10 archived packages currently eligible), locally retained online
advisory samples, partial/all-eligible Qwen advisory scoring, and append-only
ten-dimension human blind assignments. These results remain non-promotable.

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

**Implementation reached 2026-07-28:** `story-storage` now owns immutable
origin/targeted/rollback revisions, complete node correspondence, create-once
approval events, comparisons, and approved-only JSON export. `story-policy`
implements deterministic D3 repair ordering and fail-closed D4 round/budget
decisions. The desktop works from review findings through span navigation,
replacement, history, approval, comparison, rollback, and export. Storage,
desktop-service, Svelte check, and production-build evidence pass.

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

**Non-human infrastructure reached 2026-07-28:** professional evidence and
promotion-decision contracts, candidate discrimination-pair construction,
sealed-holdout commitments, pair accuracy, stratified preference bounds,
nominal professional agreement, adjudication detection, and all nine promotion
rules are implemented and tested. Missing professional reviews or screenwriter
signoff returns `non_promotable`. The exit remains intentionally unmet because
the user excluded the human blind execution; no candidate is promoted.

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

**2026-07-28 implementation status:** the non-human scope is complete. Family
and suspense draft packs, short/long constraints, agent profiles, licensed
retrieval provenance, regression manifests, and quarterly retirement policy
are schema-validated. Rust resolves the selected pack before provider access
and the unchanged fixed workflow consumes its typed context. Both packs remain
draft/non-promotable because hidden human promotion evidence is intentionally
excluded from this development pass.

---

## P9 · Production reliability and governance

- credential rotation and audit (at-rest encryption moved forward to P5);
- crash recovery, event-log repair, backup and schema migration;
- concurrency, budget, timeout and provider-degradation tests;
- structured diagnostics with secret and chain-of-thought redaction;
- dependency/license inventory, security review and incident runbook.

**Exit:** fault-injection, migration, backup/restore, security and sustained-run
checks pass with no loss of durable story or approval state.

**2026-07-28 implementation status:** migration, interrupted-write repair,
hash-verified backup/restore, restart recovery, result-before-terminal
durability, credential rotation audit, budget/timeout/provider/concurrency fault
tests, redacted diagnostics, dependency inventory, security review, incident
runbook, and a 250-run/750-event sustained check are implemented. The pinned
Campaign revision now carries owner-selected MIT evidence; live-provider soak
evidence still requires rotated credentials.

A bounded desktop live-provider soak runner is now implemented for the missing
evidence: it preflights both configured routes, performs 3–20 paid iterations
per provider, and atomically retains only timing and success/failure summaries.
No live result exists until rotated credentials are configured.

---

## P10 · Stable story-studio release

- Windows packaging and reproducible build evidence; Authenticode remains
  optional for a future public distribution channel;
- first-run provider configuration and health diagnostics;
- accessibility, localization and operator documentation;
- upgrade/rollback compatibility policy;
- stable contract/version declarations and release notes.

The stable release remains a story-writing product. Video download, video
material extraction and automatic publishing require a separate future scope
decision.

**Exit:** a clean machine can install, configure, complete and export a story
job, upgrade safely, and reproduce the release evidence.

**2026-07-28 implementation status:** actual MSI and NSIS installers build with
the bundled PyInstaller onedir sidecar under pinned Rust 1.88.0, Node 22.14.0
and Python 3.12.10. The MSI administrative extraction succeeded, and its
extracted sidecar passed the real duplicate-Start, `Last-Event-ID` replay and
idempotent-Cancel process test with no process left behind. The extracted
desktop executable also survived a five-second local launch smoke. These checks
now run inside every release build; the schema requires all three successes and
the extracted binary hashes. Deterministic lockfile build scripts, optional
Authenticode sign/verification, release hashes/toolchain/source-state evidence
(including untracked source), first-run credential routing, live provider
health checks, accessibility/locale notes, operator guide, upgrade/rollback
policy, stable contract declarations, and release notes are implemented. The
secure CI PFX import/cleanup path, Windows SDK signtool discovery, HTTPS
timestamping, and signed provenance contract are also implemented and fail
closed without secrets. Distribution-license inventory is now a pre-package
admission gate: unknown licenses stop normal/signed builds, while the explicit
unsigned local override records the unresolved dependency and an ineligible
installer. Owner-selected MIT evidence now clears the Campaign distribution
review, and the sidecar build verifies the installed exact git revision before
PyInstaller runs. The owner accepts unsigned installers for this personal
project; absence of a signing identity is no longer a P10 blocker. Current local
artifacts remain dirty and have not completed clean-VM
install/upgrade/rollback evidence, so the exit is not yet claimed.

---

# Beyond the first stable release

P10 is the first shippable product. Everything below is post-1.0, and the
confidence behind it decays fast with distance.

**Read these as a register of designed-but-unscheduled work, not as a schedule.**
Their purpose is to stop capability that is already specified from falling off
the map. Two things had already fallen off before this section existed:

- `STORY_MULTI_AGENT_DESIGN.md` §12–13 fully specifies the nanocodex skill
  derivation loop — human revision corpus, candidate `SKILL.md`, promotion gate,
  signed registry. It appeared nowhere in P0–P10. The registry is empty and all
  three draft templates carry `skills: []`, so today a template binds constraints
  and retrieval and nothing else.
- `STORY_EVAL_DESIGN.md` §11 requires expanding to 120 cases **before** enabling
  automatic skill promotion. The set is 30, and no phase owned the expansion.

The project is currently entering P2.5/P3b advisory prototype work while P1
human calibration remains deferred. Nothing below should be treated as a
commitment.

---

## P11 · Evaluation at scale

Prerequisite for P12: the parent contract forbids automatic skill promotion on a
30-case set.

- expand the case set from 30 to the 120-case target distribution —
  **done 2026-08-27, pulled ahead of the first freeze** (REQ-326) so one
  freeze covers the target scale instead of a MAJOR bump right after;
- add a **non-Chinese-native judge family**, so `inter_model_agreement` stops
  measuring shared priors between two Chinese-native models;
- MinHash / embedding premise-family checking — **done 2026-08-27**
  (`eval/tools/check_premise_families.py`): 120 cases machine-checked,
  0 cross-family near-duplicates at threshold 0.5; 25 families whose members
  share mechanism but zero surface text, recorded as naming-level info;
- similarity search of every premise against existing screen works;
- re-freeze the manifest and rubric at the larger scale.

**Exit:** 120 cases frozen, premise-family separation machine-checked, and
cross-family judge agreement measurable rather than merely reported.

---

## P12 · Skill derivation and promotion

Implement the loop `STORY_MULTI_AGENT_DESIGN.md` §12–13 already specifies:

- human revision corpus with `before`/`after`, `problem_code` and span refs;
- nanocodex analyzer producing candidate `SKILL.md` with evidence and rationale;
- lint, size and security validation of candidates;
- train → validation → hidden holdout promotion, human-gated;
- signed registry `skills-registry/<name>/<semver>/`, rollback by pointer;
- templates bind real signed skill versions instead of `skills: []`.

Gated on **P7** for the revision corpus — the pairs come from professional edits,
which do not exist until screenwriters are engaged — and on **P11** for scale.

Skill evolution stays offline and never mutates production skills directly.

**Exit:** one skill is promoted from a real revision corpus through the hidden
gate, signed, loaded by a template, and rollback is exercised.

---

## P13 · Deliverable formats

A `story-package/v1` is a JSON document. Nobody shoots from JSON.

- export to a real screenplay format for the production side;
- per-episode shooting-oriented breakdowns respecting the declared production
  tier;
- round-trip integrity: exported artifacts trace back to node ids and revisions.

This is the gap between "the system produced a story" and "a crew can use it".
It is listed after P12 only because skills change what gets produced, not
because export is less important.

**Exit:** an exported deliverable is accepted by someone who would actually
shoot from it, and traces back to the package it came from.

---

## P14 · Second content form — knowledge/explainer

The first real test of P2.5's claim. Adding a form must supply an artifact
schema, a rubric and a case set, and touch no aggregation, gating or verdict
code.

- explainer artifact schema: no character arcs, no scene dialogue;
- its own rubric — factual soundness, explanatory clarity, watch drive — with
  its own critical dimensions;
- its own case set and admission gates;
- its own adversarial seeding recipes; the drama masking recipes do not carry
  over.

If this phase requires editing `story-eval`, P2.5 failed and the failure is
worth recording plainly.

**Exit:** an explainer job runs end to end and is scored, with zero changes to
aggregation, gating or verdict logic.

---

## P15 · Third content form — real-creator

Real-creator content is not scripted output at all: single presenter, location
sound, non-fiction. The artifact is closer to a shooting plan than a script, and
the rubric has no dialogue-subtext dimension to measure.

Sequenced after P14 because P14 establishes whether adding a form is genuinely
configuration-only. Doing both at once would confound that answer.

**Exit:** three content forms coexist under one product with three rubrics and
one aggregation implementation.

---

## P16 · Video scope decision

**A decision gate, not a work phase.** P10 states that video download, material
extraction and automatic publishing require a separate scope decision. That
decision belongs here, made explicitly rather than by drift.

What must be settled before any video work starts:

- rights and licensing for downloaded material, which is the binding constraint,
  not the engineering;
- whether video output is evaluated at all, and if so against what — the entire
  evaluation apparatus assumes text artifacts;
- whether this remains one product or becomes a second one.

Existing video research documents are marked deferred and are reference only.
They are not a plan and must not be treated as one.

**Exit:** a recorded decision with its rights analysis, or a recorded decision
not to proceed.

---

## P17–P18 · Deliberately unwritten

Two slots are left empty rather than filled.

Everything from P11 onward already rests on a product that does not exist yet;
by P17 the assumptions compound past the point where writing phases produces
information. Placeholder phases invite false precision — they get counted,
scheduled and reported against, and none of that is grounded.

What would have to be true before P17 can be written honestly: P10 has shipped,
real users have run real jobs, and the failure modes are observed rather than
predicted. Until then the useful artifact is the backlog above, not more phases.

Part of planning is refusing work that is not worth its cost now.

| Item | Decision |
| --- | --- |
| `docs/eval-governance.html` | Stop hand-maintaining it. It is a derived view of the manifest, so manual upkeep guarantees recurring drift — the current `pair_accuracy` / `seeded_defect_detection` inconsistency is the second instance. Regenerate it from the manifest instead, or delete it. |
| Similarity search of the 30 premises against existing screen works | Defer until just before freezing. Nothing is in production, so the cost is real and the payoff is not yet. |
| MinHash / embedding family checking | Defer to 120 cases. At 30, reading them is sufficient and has already caught two collisions that label inspection missed. |

---

## Historical measured status of P10-P18 — verified 2026-07-29

> Historical snapshot only. It predates the 2026-08 media-agent work; current
> ownership and interfaces live in `docs/project-memory/PROJECT_REGISTRY.yaml`
> and feature traceability files.

Moved here from `HANDOFF.md`. Phase status belongs to the roadmap; the
handoff should carry only what the next session must act on. Every figure
below was read off the repository, not inferred from plan documents.

**P10：工程实现完成，Exit 条件未满足。** 打包链路是真的（MSI/NSIS 实际生成、
sidecar 内嵌、first-run 配置页、许可证清单 fail-closed、evidence 绑定），但 Exit
是「clean machine 安装→配置→完整故事→导出→升级→回滚」，该项未跑。

**P11-P16：零实现，当前只是登记册。** 逐项实测：

| 阶段 | 目标 | 实测 |
|---|---|---|
| P11 评测规模化 | 120 例 | **30 例**（dev 10 / train 9 / validation 7 / holdout 0 / challenge 4） |
| P12 技能闭环 | 签名注册表 + 模板绑技能 | `skills-registry/` **不存在**；**8 个 genre pack 的 `skills` 全部为空数组** |
| P13 交付格式 | 可拍的剧本格式 | 无 |
| P14 科普形态 | 第二种 content form | `genre-template-v1.json` 的 `content_form` 枚举**只有 `scripted_short_drama` 一个值** |
| P15 真人形态 | 第三种 | 同上 |
| P16 视频范围决策 | 一份决策记录 | 当时无；`story-media` crate 当时尚未创建。当前已有 provider-neutral image/video execution seam，但素材导入/FFmpeg 管线仍 deferred。 |

P12 那行需要单独注意：**8 个 genre pack 全部 `skills: []`**，意味着 pack 目前只
绑硬约束与检索集合，**技能层是空的**——而技能正是系统自我改进的载体。

**P17-P18：不存在，刻意留空。** 理由见 `docs/ROADMAP.md`：P11 起已建立在尚不
存在的产品上，占位阶段会被计数、排期、汇报，制造虚假精度。

**P11-P16 当前全部动不了**：P12 卡在 P7（编剧）与 P11（120 例），P14 卡在 P2.5
剩余两件，P16 是决策不是工程。真正挡路的仍是三件——P1 退出条件失败、DAG 从未
跑过真实故事、零份人类证据。

---

## Dependency summary

```text
P0 seal
  └─ P1 engineering complete; human calibration deferred
       └─► P2.5 form registry
             └─► P3b advisory runtime ──► first real artifact
                    ├─► deferred human check ──► P3a freeze
                    │       └─► repeat P3b against frozen eval
                    └─► P5 desktop prototype
                              └─► P4/P6/P8/P9/P10 only after their own gates

P3a freeze ──► P7 professional gate
               (parallel external lead time; required for promotion)
```

Post-1.0, the register continues:

```text
P10 stable ──► P11 eval at scale ──► P12 skill loop (also needs P7)
           ──► P13 deliverable formats
           ──► P14 explainer form ──► P15 real-creator form
           ──► P16 video scope decision
               P17-P18 deliberately unwritten
```

P1 human calibration is now a delayed freeze/promotion gate rather than a
prototype-start gate. P3b is the first phase that produces a real story
artifact, but its first output is explicitly non-promotable. P5 makes the
prototype usable without a developer console. P10 is the stable release. P12
is the first phase where the system improves itself, and it cannot start until
professionals have edited real output.

**P7 is drawn as a parallel branch on purpose.** It is the only phase gated on
recruiting people rather than writing code, and it is the only one that can
promote a candidate. Sequencing it behind the desktop and the revision workflow
would let the product reach feature completeness having never been read by
anyone qualified to judge it.
