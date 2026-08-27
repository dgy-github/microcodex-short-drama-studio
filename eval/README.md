# Evaluation Workspace

The target-state contract is
[`../docs/STORY_EVAL_DESIGN.md`](../docs/STORY_EVAL_DESIGN.md).

The version that is actually runnable now, without a professional screenwriter
panel, is [`../docs/STORY_EVAL_V1.md`](../docs/STORY_EVAL_V1.md). Adversarial
set construction is in
[`../docs/STORY_EVAL_ADVERSARIAL.md`](../docs/STORY_EVAL_ADVERSARIAL.md).

Cases remain split by premise family and source lineage. Hidden holdout prompts,
human references, and review notes must not enter model or skill-generator
context. `holdout` is not consumed in v1 and stays sealed.

## Layout

```text
manifests/eval-v0.1.0.json   thresholds; owns every number, versioned with the set
rubrics/judge-v1.yaml        shared by LLM judges, internal spot checks, and the
                             future professional panel
cases/<split>/cases.jsonl    eval-case/v1 records
adversarial/pairs.jsonl      eval-adversarial-pair/v1 records
runs/<run_id>/               scores, evaluator metrics, summary
```

Aggregation and floors are implemented in `crates/story-eval`. v1 output is
advisory and does not gate production promotion.

## Splits

The set contains 120 internally authored prompts at the parent contract's
target distribution (`dev:train:validation:challenge` = 30:30:24:12):

```text
dev 38   train 37   validation 30   challenge 15   holdout 0 (sealed in v1)
```

Genre allocation is the 120-case target distribution, and it is a property of
the **whole set**, not of any single split:

```text
family 20; urban_romance 16; revenge 16; suspense 16;
workplace 12; rural 12; comedy 12; historical 8; cross_genre 8
```

`train` carries `skill_derivation` rights; every other split is licensed for
`evaluation` only. A train case licensed only for evaluation cannot legally
reach nanocodex, which is the entire purpose of that split.

Allocation lives in a table in `eval/tools/split_cases.py` rather than in the
files themselves, because the invariant that matters — cases sharing a
`premise_family` never separate — cannot be maintained by hand. Families name
the underlying dramatic mechanism, not the props: a parcel locker, a laundry
token and a bus ticket are one family when all three are "an impossible trace
appears in a monitored space with records showing no entry".

## Baselines

`.gitignore` excludes `/eval/runs/`, so anything generated there exists only on
the machine that paid for it, and generation is stochastic — regenerating does
not reproduce it. Baseline artifacts are therefore archived into the tracked
`eval/baselines/<run_id>/`:

```text
eval/baselines/<run_id>/index.json          model, seed, temperature, per-case hashes
eval/baselines/<run_id>/<case>.story-package.json
eval/baselines/<run_id>/<case>.artifact.json
```

Raw provider responses stay behind in `eval/runs/`. They are run telemetry,
carry usage and billing metadata, and are not evaluation inputs.

```powershell
python eval/tools/archive_baselines.py
python eval/tools/archive_baselines.py --check
```

## Validate cases

```powershell
python eval/tools/split_cases.py --check
python eval/tools/validate_cases.py
python -m unittest discover -s eval/tools -p "test_*.py"
```

`validate_cases.py` checks two scopes. Per file: fields, types, and agreement
between each record and the directory holding it. Across the union of splits:
genre quota, difficulty coverage, hard-slice markers, licence uniqueness, and
premise-family integrity. The second scope is not optional — leakage is by
definition a cross-split property, and a check that reads one split at a time
can never observe it.

Genres with at least three pilot cases cover `ordinary`, `ambiguous`, and
`hard`. The two smallest slices (`historical` and `cross_genre`) cover
`ambiguous` and `hard`; all three become mandatory when those slices expand.

## Generate model-only baselines

`generate_baselines.py` calls one model directly through an OpenAI-compatible
chat-completions endpoint. It does not use Campaign or the product runtime.

```powershell
python -m pip install -r eval/tools/requirements.txt
python eval/tools/generate_baselines.py --dry-run
$env:MODEL_API_KEY = "<provider key>"
python eval/tools/generate_baselines.py `
  --endpoint "https://provider.example/v1/chat/completions" `
  --model "provider-model-id" `
  --run-id "baseline-001"
```

Outputs are written under `eval/runs/<run-id>/`. Completed artifact pairs are
skipped on resume. Partial pairs and conflicting run configurations stop the
run, and existing artifacts are never overwritten.
## Stage-0 specificity

Stage-0 judge summaries report both `specificity_all` and
`specificity_cross_pillar`. The latter is the P2 separability decision view;
the former remains a diagnostic for within-pillar collapse. Legacy
`specificity` and `min_specificity` fields alias the all-dimension view.
