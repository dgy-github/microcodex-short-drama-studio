# Short-Drama Story Evaluation Design

## 1. Objective

This evaluation set measures whether the system can turn a short Chinese input
into an original, coherent, emotionally credible, and producible short-drama
story. It is an acceptance gate for prompts, models, agents, retrieval changes,
and nanocodex-generated skills; it is not a leaderboard optimized only for an
LLM judge.

The primary quality target is **human credibility**: characters should behave
like specific people under pressure instead of generic plot devices.

## 2. Evaluation unit

One case contains:

```json
{
  "case_id": "example_family_001",
  "split": "holdout",
  "genre": "family",
  "input": "兄妹接手一家即将停业的社区照相馆，却只允许保留一个人的经营方案。",
  "constraints": {
    "episodes": 8,
    "minutes_per_episode": 2,
    "audience": "25-45",
    "rating": "general",
    "production_level": "low_budget"
  },
  "required_elements": ["photo studio", "business plan"],
  "forbidden_elements": ["梦醒", "失忆解决一切"],
  "rights": {
    "source": "commissioned",
    "license_id": "internal-...",
    "allowed_uses": ["evaluation"]
  }
}
```

The generated artifact must contain:

1. logline;
2. genre and audience promise;
3. character cards with desire, fear, contradiction, secret, and change;
4. causal story beats;
5. episode plan with opening state, conflict, turn, and end hook;
6. two representative scripted scenes with action, dialogue, and subtext;
7. continuity ledger for facts, relationships, time, and planted/payoff items;
8. provenance manifest listing retrieved evidence IDs, never copied source text.

## 3. Corpus layers

### 3.1 Prompt set

Prompts are newly commissioned or internally authored. They must not be direct
summaries of protected works. Each genre contains ordinary, ambiguous, and hard
cases.

Initial M0 target: 120 cases.

| Genre | Cases | Required hard slice |
| --- | ---: | --- |
| family and marriage | 20 | moral conflict without a villain |
| urban emotion / romance | 16 | attraction plus incompatible interests |
| revenge / comeback | 16 | justified desire with an ethical cost |
| suspense / crime | 16 | clue chain and fair reveal |
| workplace | 12 | concrete profession and power structure |
| rural / regional life | 12 | place-specific detail without caricature |
| comedy | 12 | character-driven rather than meme-only humor |
| historical / fantasy | 8 | rules and production constraints |
| cross-genre adversarial | 8 | conflicting requirements and taboo clichés |

### 3.2 Licensed reference library

Store only content with a recorded source and allowed use:

- commissioned stories and screenplays;
- licensed works;
- public-domain works;
- user-uploaded works with an explicit rights declaration;
- redacted real-life submissions with consent.

Every document receives `source_id`, owner, license, allowed uses, expiration,
hash, and deletion status. Training, retrieval, evaluation, and display rights
are separate flags.

### 3.3 Human revision corpus

This is the highest-value learning signal:

```json
{
  "draft_id": "draft-...",
  "case_id": "example_family_001",
  "before": {"artifact_ref": "..."},
  "after": {"artifact_ref": "..."},
  "edits": [
    {
      "span_ref": "scene-2/dialogue-7",
      "problem_code": "MOTIVE_EXPLICIT",
      "reason": "人物把真实动机直接说破，失去家庭关系中的回避感",
      "principle": "让诉求通过争夺物件、沉默和转移话题显现"
    }
  ],
  "editor_id": "blind-editor-03",
  "rights": {"allowed_uses": ["evaluation", "skill_derivation"]}
}
```

Raw text and abstract principles are stored separately. Skill derivation reads
only records permitted for that use.

## 4. Splits and leakage control

Use five disjoint splits:

- `dev` 30 cases: visible during implementation;
- `train` 30 cases: visible to nanocodex when proposing skills;
- `validation` 24 cases: used by the automatic acceptance gate;
- `holdout` 24 cases: hidden prompts, human scoring, release gate;
- `challenge` 12 cases: refreshed quarterly after production failures.

Separation is by premise family, not only by exact prompt. Near-duplicate
premises, shared characters, licensed source derivatives, and paraphrases stay
in the same split. A MinHash/embedding similarity check flags suspected leakage;
human review decides ambiguous cases.

Models and skill generators never receive holdout prompts, reference answers,
review notes, or score breakdowns. They receive only aggregate pass/fail after a
candidate run.

## 5. Scoring rubric

Each dimension is scored 1–5 by a human reviewer. Anchors are mandatory:

| Dimension | Weight | Score 1 | Score 3 | Score 5 |
| --- | ---: | --- | --- | --- |
| human credibility | 20% | people act only to move plot | motives mostly work but feel familiar | choices reveal specific history, conflict, and self-deception |
| causal coherence | 12% | events are arbitrary | main chain works with gaps | every turn follows pressure, choice, and consequence |
| character distinction | 10% | interchangeable voices | cards differ more than scenes | behavior, language, and coping patterns remain distinct |
| emotional progression | 10% | flat or forced emotion | several effective beats | restraint, escalation, reversal, and release are earned |
| short-drama pacing | 10% | slow opening/no hooks | usable rhythm with weak episodes | early promise, dense turns, varied hooks, earned climax |
| dialogue and subtext | 10% | exposition and slogans | mixed natural/expository dialogue | speech hides, negotiates, attacks, and protects |
| originality | 10% | recognizable copy or trope collage | familiar premise with variation | specific recombination and non-obvious choices |
| genre fulfillment | 6% | violates audience promise | recognizable genre | fulfills and productively bends genre expectations |
| continuity | 5% | contradictions break the story | minor repairable gaps | facts, setups, relationships, and payoffs remain stable |
| producibility | 5% | impossible for requested budget | mostly shootable | dramatic value comes from achievable staging |
| safety/compliance | 2% | disallowed content | repairable concern | compliant without flattening conflict |

Human credibility, originality, and causal coherence are critical dimensions;
their low scores cannot be hidden by the weighted average.

## 6. Automatic checks

Automatic checks are filters, not substitutes for human judgment:

- artifact JSON schema and required field validation;
- episode count, duration, and constraint compliance;
- named-entity and continuity ledger consistency;
- setup/payoff reference integrity;
- repeated line and phrase detection;
- source-library similarity and long-span overlap checks;
- dialogue speaker balance and exposition-density heuristics;
- prohibited trope and safety rule checks;
- token, latency, provider, retrieval IDs, and cost accounting.

LLM judges may pre-score artifacts and explain likely defects. Their scores are
calibrated against human judgments and never decide the release gate alone.

## 7. Human evaluation protocol

### 7.1 Review panel

- three independent reviewers per formal holdout comparison;
- at least two working screenwriters or story editors and one target viewer;
- an additional adjudicator when any critical dimension differs by 2 or more points;
- model/skill/version identity hidden from reviewers;
- output order randomized and candidate names removed.

### 7.2 Pairwise comparison

For every candidate change, compare candidate and incumbent on the same cases.
Reviewers answer:

1. Which story would you continue watching?
2. Which characters feel more like real people?
3. Which version is more original?
4. Which version is more producible?
5. Is either version unacceptable, and why?

Pairwise preference is the primary release statistic; absolute rubric scores
provide diagnosis.

### 7.3 Calibration

Before scoring production candidates, reviewers score ten anchor artifacts with
agreed explanations. Track Krippendorff's alpha and dimension-level drift; the
initial alpha target is `>= 0.67`. If agreement falls below the configured
threshold, recalibrate before continuing.

## 8. Acceptance gates

A candidate model, prompt, agent graph, retriever, or skill version is promoted
only when all conditions hold:

1. schema and policy checks pass on every case;
2. no critical-dimension mean decreases by more than `0.10`;
3. no genre slice loses more than `0.15` weighted points;
4. holdout pairwise preference lower confidence bound is above `50%`;
5. critical failure count does not increase;
6. originality overlap gate has zero blocking violations;
7. mean cost and p95 latency stay inside the declared budget, or the quality
   gain is explicitly approved;
8. at least one screenwriter signs off on the blinded holdout summary.
9. stochastic generation is evaluated across at least three declared seeds or
   repeated samples; no candidate may select only its best output.

For M0, confidence intervals use stratified bootstrap by case. Thresholds are
configuration, versioned with the evaluation set, and may be tightened after
the first calibration run.

## 9. Failure taxonomy

Every rejection uses one or more stable codes:

- `HUMAN_GENERIC`: generic behavior without lived specificity;
- `MOTIVE_EXPLICIT`: characters state their true motive instead of dramatizing it;
- `PLOT_CONVENIENCE`: coincidence or sudden information resolves pressure;
- `VOICE_COLLAPSE`: characters share one voice;
- `EMOTION_UNEARNED`: emotional turn lacks accumulated cause;
- `HOOK_FAKE`: cliffhanger has no consequence in the next episode;
- `TROPE_STACK`: clichés are stacked without transformation;
- `EXPOSITION`: dialogue explains facts all participants already know;
- `CONTINUITY`: fact, timeline, relationship, or setup/payoff contradiction;
- `UNSHOOTABLE`: violates production constraints;
- `SOURCE_OVERLAP`: suspicious similarity to a protected reference;
- `POLICY`: rights, safety, or customer constraint violation.

These codes feed analysis and skill proposals. Free-text notes remain available
to human reviewers but are not blindly injected into production prompts.

## 10. Versioning and artifacts

```text
eval/
  manifests/eval-v0.1.0.json
  cases/{dev,train,validation,holdout,challenge}/
  rubrics/human-v1.yaml
  anchors/calibration-v1/
  runs/<run_id>/
    config.json
    artifacts/
    automatic_scores.jsonl
    blind_review_assignments.json
    human_scores.jsonl
    summary.json
```

Each run records model IDs, prompts/skill hashes, Campaign commit, nanocodex
commit, retrieval snapshot, random seeds where supported, costs, and reviewer
rubric version.

## 11. M0 delivery

1. Author 30 pilot cases across six genres.
2. Produce one incumbent and one deliberately weak artifact per case.
3. Calibrate three reviewers and revise anchors/rubric.
4. Run a blinded model-only versus multi-agent comparison.
5. Freeze `eval-v0.1.0` only after agreement and leakage checks pass.
6. Expand to 120 cases before enabling automatic skill promotion.
