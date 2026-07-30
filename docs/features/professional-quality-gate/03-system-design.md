# Professional quality gate system design

Status: G6 non-human infrastructure passed; human execution excluded

Private holdout files remain outside tracked and model-readable paths. The seal
tool publishes only counts and a deterministic SHA-256 commitment. Verification
recomputes the commitment from the private directory.

The promotion gate consumes typed professional evidence, validates panel
composition and blindness, computes pair-majority accuracy, a stratified
bootstrap lower confidence bound, nominal Krippendorff agreement, and required
adjudications. It then evaluates all release constraints from
`STORY_EVAL_DESIGN.md`.

An empty or incomplete human evidence set produces `non_promotable`. Metric
failure after structurally complete human evidence produces `reject`. Only
complete human evidence passing every condition can produce `promote`.
