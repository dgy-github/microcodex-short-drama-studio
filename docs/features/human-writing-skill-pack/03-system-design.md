# Human writing skill pack design

Status: G2 contracts ready

## Ownership

Rust `story-runtime` extends the existing genre configuration owner. It loads
and validates one `human-writing-profile/v1`, then includes its task-scoped
directives in the typed `GenreContext`. Python receives those directives as a
bounded capability input and cannot select files or arbitrary prompts.

## Profile

The profile contains five non-empty directive lists keyed by `t07`, `t10`,
`t12`, `t15` and `t16`. Rules focus on distinguishable character voice,
subtext, lived detail, emotion expressed through behavior, imperfect but
motivated choices and evidence-based anti-pattern review.

## Runtime injection

`build_prompt` appends only the current task's directives. The bounded episode
room uses the `t10` list for every episode child. No new model call is added,
so the fixed 17-task topology and provider routing remain unchanged.

For `t15`, the runtime compacts revision input to character, beat and episode
structures plus cited review findings. When no finding cites a scene, the model
does not receive or reproduce the completed episode prose; `canonical_package`
preserves and assembles the existing `t10` scenes. This avoids paying twice for
the full story while keeping revisions evidence-driven.

## Trust and release

The configuration is project-authored and contains no external retrieval
source. Human-taste findings remain advisory and subject to the existing
hidden-human promotion gate.
