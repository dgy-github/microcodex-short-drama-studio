# Candidate decision retention — system design

Status: G2 contracts ready

`story-policy` owns ranking. `story-storage` owns durable persistence. The
runtime constructs a `CandidateDecisionTrace` only after ranking has completed,
validates it, and passes the complete trace to `CandidateDecisionStore`.

The storage call is atomic at the decision level and idempotent by
`decision_id`. Partial candidate sets are invalid because they would bias the
future rank-correlation calculation while looking structurally usable.

The trace stores:

- job, run and decision identity;
- exact online policy version;
- candidate ID and immutable artifact hash;
- final online score;
- one-based deterministic rank;
- selected or rejected disposition.

Defect reasons stay in the policy/review artifacts referenced by artifact hash.
The retention boundary does not duplicate policy calculation or offline scores.

## Failure handling

- Invalid traces are rejected before persistence.
- A duplicate decision ID must be treated idempotently by an implementation.
- Storage failure fails the `t06` durability step; the runtime must not emit a
  durable selection-completed event first.
