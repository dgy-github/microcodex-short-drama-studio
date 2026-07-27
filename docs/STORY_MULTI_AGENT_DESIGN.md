# Short-Drama Multi-Agent Technical Design

## 1. Decision

The story-generation subsystem uses:

- Campaign at pinned commit `6f7d0030b127c699ec5b6324b77795ed3a2452e0`
  for command, DAG execution, routing, governance, events, review, and recovery;
- the Rust/Tauri application for trusted storage, provider calls, rights checks,
  accounting, and user interaction;
- nanocodex for offline analysis and candidate `SKILL.md` generation;
- ncx-forge-style train/validation/holdout gates for skill promotion.

Hermes Agent is not part of the architecture.

## 2. Trust and process boundaries

```text
Svelte story workspace
  -> Tauri IPC
Rust StoryService
  -> validates request, rights, budget, provider preset
  -> starts pinned Campaign sidecar
Campaign Runtime
  -> ExecutionOrder / Task DAG / EventLog / PolicyGate / Reviewer
  -> async commands + append-only events + authenticated SSE
Rust Capability Host
  -> retrieval / model / schema / similarity / persistence
Licensed corpus + run artifacts

Offline:
human revisions -> nanocodex analyzer -> candidate SKILL.md
-> evaluation runner -> human holdout gate -> signed skill registry
```

Campaign receives logical capabilities, not shell commands. Only Rust can read
customer projects, invoke providers, or persist production artifacts.

## 3. Product job contract

The customer-facing schema is product-owned and independent of Campaign:

```json
{
  "schema": "story-job/v1",
  "job_id": "job_...",
  "input": "一句或一段故事种子",
  "genre_mode": "auto",
  "allowed_genres": ["family", "suspense"],
  "audience": "25-45",
  "format": {"episodes": 12, "minutes_per_episode": 2},
  "production": {"tier": "low_budget", "max_locations": 6},
  "content_limits": [],
  "retrieval_policy": {
    "collections": ["licensed-story-patterns", "human-detail-notes"],
    "allow_full_text": false
  },
  "budget": {"max_tokens": 180000, "max_cny": 12, "deadline_s": 600}
}
```

Rust translates this into a Campaign `ExecutionOrder`. Campaign identifiers are
stored as operational metadata but never replace `job_id`.

## 4. Agent registry

Campaign's fixed roles are retained; product specialization is expressed
through `AgentSpec.skills`.

| Agent ID | Campaign role | Skills | Responsibility |
| --- | --- | --- | --- |
| story-coordinator | coordinator | genre-routing, story-dag | choose genre lane and build/repair the DAG |
| genre-analyst | executor | genre-classification | classify promise, audience, tone, and risk |
| life-detail-retriever | retriever | licensed-rag, provenance | retrieve human details and structural patterns |
| story-architect-a/b/c | executor | premise, character, beats | independently propose three story architectures |
| character-room | executor | character-arc, relationship | deepen motives, contradictions, secrets, and voices |
| episode-planner | executor | episode-hooks, pacing | turn the selected architecture into episode beats |
| scene-writer | executor | scene, dialogue, subtext | produce representative scenes and dialogue |
| continuity-editor | reviewer | causality, continuity | find fact, motive, timeline, and setup/payoff defects |
| human-taste-editor | reviewer | human-credibility | reject generic behavior and unearned emotion |
| originality-editor | reviewer | provenance, similarity | enforce source and overlap policy |
| production-editor | reviewer | producibility | enforce locations, cast, duration, and budget |
| reserve-writer | reserve | takeover, repair | replace failed workers without changing acceptance |

Multiple specialized reviewers may run as executor tasks that produce reports;
the final Campaign `Reviewer` aggregates them and is configured fail-closed.

## 5. ExecutionOrder template

```text
t01 classify_genre
t02 retrieve_evidence         depends: t01
t03 propose_architecture_a    depends: t01,t02
t04 propose_architecture_b    depends: t01,t02
t05 propose_architecture_c    depends: t01,t02
t06 debate_and_select         depends: t03,t04,t05
t07 deepen_characters         depends: t06
t08 build_story_beats         depends: t07
t09 plan_episodes             depends: t08
t10 write_sample_scenes       depends: t09
t11 continuity_review         depends: t08,t09,t10
t12 human_taste_review        depends: t07,t09,t10
t13 originality_review        depends: t02,t08,t10
t14 production_review         depends: t09,t10
t15 targeted_revision        depends: t11,t12,t13,t14
t16 final_review              depends: t15
t17 package_artifact          depends: t16
```

Every task has structured acceptance criteria. Example:

```json
{
  "id": "t12",
  "goal": "评估人物是否像具体的人，而不是推动情节的工具",
  "required_skills": ["human-credibility"],
  "acceptance": "返回 human-taste-review/v1；列出证据位置；critical defects=0",
  "depends_on": ["t07", "t09", "t10"]
}
```

The Coordinator may repair dependencies or retry a failed lane, but it may not
weaken acceptance criteria, expand source rights, or increase budget.

## 6. Candidate generation and debate

Architecture proposals A/B/C run independently with different declared lenses:

- A: strongest emotional relationship;
- B: strongest plot engine and episode hooks;
- C: most specific lived context and least conventional choice.

They share only the input contract and retrieved evidence manifest. They do not
see one another's prose before submission, reducing convergence.

`debate_and_select` receives compact structured proposals, not hidden reasoning.
It must:

1. identify the strongest causal engine in each proposal;
2. identify generic or derivative elements;
3. select one proposal or explicitly combine named components;
4. record rejected alternatives and reasons;
5. emit a decision object that downstream tasks can audit.

No chain-of-thought is persisted. Only decisions, evidence references, defects,
and revisions enter the event log.

## 7. Asynchronous event and SSE protocol

### 7.1 Communication rule

Multi-agent communication is asynchronous by default:

- commands are accepted and queued; they do not wait for the complete result;
- every meaningful transition is appended as an immutable domain event;
- SSE is required for cross-process progress, content deltas, approvals,
  artifacts, review findings, and terminal outcomes;
- synchronous request/response is limited to health, capability discovery,
  command acceptance, and current-state snapshots;
- polling is a recovery fallback, never the normal progress path.

Campaign's A2A `Message`, `Part`, `run_id`, `task_id`, and `correlation_id`
remain the agent envelope. In-process agents publish through bounded async
queues backed by `EventLog`; remote agents use Campaign's `message/stream` SSE
transport. Rust exposes the same event semantics to Tauri and the frontend
instead of translating progress into ad-hoc callbacks.

### 7.2 Command path

Commands use authenticated localhost HTTP/JSON-RPC:

```text
POST StartRun / ResumeRun / CancelRun / SubmitHumanInput
  -> validate identity, rights, schema, budget, and idempotency key
  -> append command.accepted or command.rejected
  -> enqueue work
  -> return 202 Accepted {request_id, job_id, run_id, event_stream_url}
```

Returning `202` means accepted for processing, not completed. Repeating a
command with the same idempotency key returns the original acceptance record and
never creates duplicate paid work.

### 7.3 SSE path

```text
GET /v1/runs/{run_id}/events
Accept: text/event-stream
Last-Event-ID: <last durable sequence>
```

Each SSE frame has an event name, durable sequence ID, and JSON data:

```text
id: 1842
event: task.output.delta
data: {"protocol":"story-agent-event/v1","run_id":"...","task_id":"t10","seq":1842,"payload":{"text":"..."}}
```

SSE requirements:

- event IDs increase monotonically within one `run_id`;
- reconnect resumes strictly after `Last-Event-ID`;
- replay comes from the durable EventLog before live subscription begins;
- delivery is at least once, so consumers deduplicate by `run_id + seq`;
- heartbeat comments are sent every 15 seconds and do not advance sequence;
- disconnect does not imply task failure;
- slow consumers may receive coalesced non-durable deltas, but durable state,
  approval, artifact, review, error, and terminal events are never dropped;
- a stream is not blindly retried mid-response; the client reconnects from its
  last durable event ID;
- large artifacts are sent by reference, not embedded as complete long texts.

### 7.4 Event envelope

All durable events contain:

```json
{
  "protocol": "story-agent-event/v1",
  "event_id": "evt_...",
  "seq": 1842,
  "occurred_at": "2026-07-26T12:00:00Z",
  "causation_id": "cmd_...",
  "correlation_id": "req_...",
  "job_id": "job_...",
  "run_id": "campaign-run-id",
  "task_id": "t03",
  "agent_id": "story-architect-a",
  "event_type": "task.artifact.ready",
  "schema_version": 1,
  "payload": {}
}
```

Required commands:

- `StartRun`
- `ResumeRun`
- `CancelRun`
- `SubmitHumanInput`
- `GetRunState`
- `CapabilityRequest`

Required event families:

- `run.accepted`, `run.started`, `run.completed`, `run.failed`, `run.cancelled`;
- `task.queued`, `task.started`, `task.output.delta`, `task.artifact.ready`;
- `task.input.required`, `task.resumed`, `task.completed`, `task.failed`;
- `review.finding`, `review.completed`, `revision.requested`;
- `budget.updated`, `policy.rejected`, `source.revoked`;
- `agent.available`, `agent.degraded`, `agent.replaced`.

`ProgressEvent`, `ApprovalRequired`, `TaskArtifactReady`, `RunCompleted`, and
`RunFailed` are event projections, not synchronous RPC calls.

`CapabilityRequest` uses an allowlisted union:

- `classify_text`
- `retrieve_authorized_chunks`
- `generate_structured_text`
- `validate_artifact`
- `check_similarity`
- `load_skill`
- `store_artifact`

Unknown capability names, paths outside the job workspace, expired licenses,
and unbudgeted provider requests are rejected before execution.

### 7.5 Ordering and consistency

- ordering is guaranteed within a `run_id`, not globally;
- events from concurrent tasks may interleave and retain their `task_id`;
- tasks may emit deltas before persistence, but downstream tasks consume only
  `task.artifact.ready`;
- terminal state is derived from durable events, never connection close;
- the product database stores its own projection and last consumed sequence;
- approval/cancellation races use the first durable terminal decision; later
  conflicting commands are rejected and logged.

### 7.6 Tauri and UI projection

Rust owns one SSE consumer per active run and converts sidecar events into typed
Tauri events. Svelte never connects to the Python sidecar directly. UI rendering
may batch `task.output.delta` frames at 30–60 ms; state, approval, review, cost,
and terminal events render immediately.

## 8. Artifact schemas

Each task produces a typed artifact:

- `genre-analysis/v1`
- `retrieval-manifest/v1`
- `story-architecture/v1`
- `character-bible/v1`
- `story-beats/v1`
- `episode-plan/v1`
- `sample-scenes/v1`
- `continuity-review/v1`
- `human-taste-review/v1`
- `originality-review/v1`
- `production-review/v1`
- `revision-plan/v1`
- `story-package/v1`

Artifacts are immutable and content-addressed. A revision creates a new artifact
with `supersedes` and defect IDs; it does not overwrite evidence needed for
evaluation or audit.

## 9. State model and recovery

Product state:

```text
draft -> validating -> queued -> running
running -> input_required | reviewing | failed | cancelled
input_required -> running | cancelled
reviewing -> revising | completed | failed
revising -> reviewing
failed -> retrying -> running
```

Campaign events remain the operational source for task progress. Rust derives
customer-visible state and stores the last consumed event sequence. On restart:

1. load the product job;
2. replay Campaign events by `run_id`;
3. reconcile immutable artifacts by hash;
4. mark in-flight provider calls unknown until idempotency lookup completes;
5. resume only tasks whose paid side effects are known not to have completed.

After replay, Rust reconnects SSE using the last durable sequence. A disconnected
stream never marks a task failed; only a durable failure or timeout event does.

Every provider request has an idempotency key derived from job, task, artifact
input hashes, model preset, and skill version.

## 10. Retrieval and rights policy

The retriever never searches an unrestricted web corpus during generation.
Collections are explicitly selected by the product policy.

Retrieved records include:

```json
{
  "source_id": "src_...",
  "chunk_id": "chunk_...",
  "rights": ["retrieval", "skill_derivation"],
  "kind": "human-detail-note",
  "text": "...",
  "hash": "...",
  "expires_at": null,
  "untrusted": true
}
```

Rules:

- full protected text is excluded unless the license explicitly permits it;
- generation context prefers abstract annotations, beats, and authorized short
  evidence over entire works;
- retrieved text is delimited as untrusted data and cannot issue instructions;
- every output stores source IDs but does not expose licensed text by default;
- removal of a source invalidates future retrieval snapshots and derived skills
  that cannot retain their legal basis.

## 11. Review and revision policy

The final Campaign Reviewer is fail-closed. Completion requires:

- all required artifacts validate;
- no unresolved critical defects;
- human-credibility, causal, originality, continuity, and production reviews pass;
- source overlap is below the blocking threshold;
- cost and rights policies pass.

Campaign's compatibility behavior may accept an absent reviewer or malformed
review output. The story adapter must override that behavior:

- call `set_require_reviewer(true)`;
- validate every review against the product review schema;
- treat missing, unparsable, timed-out, or exception-producing reviews as
  rejection;
- prohibit the stub/no-model reviewer path in production and release evaluation.

Revision is targeted. Each defect identifies artifact path, evidence, severity,
and requested change. The revision task may edit only affected sections plus
dependent continuity fields. Maximum revision rounds default to two; exceeding
the limit returns `input_required` instead of silently spending more.

## 12. nanocodex skill derivation

Skill evolution is offline and never mutates production skills directly.

Input:

- failed run artifacts;
- human revision pairs;
- stable failure codes;
- reviewer explanations permitted for skill derivation;
- current skill and its version.

nanocodex task:

1. group repeated failures by genre and failure code;
2. compare before/after artifacts;
3. propose a minimal rule with positive and negative examples;
4. update one candidate `SKILL.md`;
5. include source record IDs, rationale, and expected affected slices.

Candidate layout:

```text
skills-candidates/<proposal_id>/
  SKILL.md
  manifest.json
  evidence.jsonl
  diff.md
```

The manifest records data rights, nanocodex/model version, parent skill hash,
affected genres, and expiry/revocation dependencies.

## 13. Skill promotion gate

```text
candidate SKILL.md
-> lint and size/security validation
-> train cases: must improve targeted failures
-> validation: weighted score and critical dimensions must improve/not regress
-> hidden holdout: blinded pairwise human review
-> rights/provenance review
-> signed promotion
-> immutable registry release
```

Promotion follows ncx-forge's incumbent/candidate and holdout principles but
uses the story rubric in `STORY_EVAL_DESIGN.md`. Automatic model-judge lift is
insufficient. Production loads only signed versions from:

```text
skills-registry/<skill-name>/<semver>/
  SKILL.md
  manifest.json
  signature.json
```

Rollback switches the active registry pointer; historical runs retain the exact
skill hash.

Initial promotion targets are a validation weighted-score lift of at least 3%,
a human-credibility pairwise lift of at least 5%, no critical genre regression
over 2%, and cost growth below 15% unless explicitly approved. The evaluation
manifest owns these thresholds; this document does not silently change them.

## 14. Observability

Required per-run metrics:

- end-to-end and per-task latency;
- prompt, completion, and total tokens by provider/model;
- estimated and actual CNY cost;
- retry, timeout, approval, and reviewer rejection counts;
- retrieval source IDs and overlap score;
- candidate diversity before selection;
- defect counts by taxonomy and revision round;
- human pairwise preference when evaluated;
- exact Campaign, nanocodex, model, prompt, and skill versions.

Persist summaries separately from potentially sensitive artifacts.

## 15. M0 implementation sequence

1. Define product job and artifact JSON Schemas.
2. Implement Rust sidecar lifecycle and authenticated localhost transport.
3. Implement async command acceptance, EventLog replay, SSE resume, heartbeat,
   deduplication, and backpressure tests.
4. Adapt one fixed `ExecutionOrder`; do not enable free-form decomposition yet.
5. Register genre, three architect, episode, scene, and three reviewer lanes.
6. Implement licensed retrieval manifest and provenance checks.
7. Run the 30-case evaluation pilot.
8. Add nanocodex candidate skill generation without automatic promotion.
9. Enable resume/idempotency and chaos tests.
10. Only then expose the story workflow in the desktop UI.

## 16. Explicit non-goals for M0

- scraping unlicensed novels;
- autonomous production skill mutation;
- model weight fine-tuning;
- unrestricted web retrieval;
- LLM-generated shell execution;
- direct video generation;
- replacing human holdout review with an LLM judge.
