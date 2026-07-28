# REQ-301 — Retain every online selection candidate

Status: approved for P4 implementation

## Problem

At `t06`, the runtime selects one story architecture from several candidates.
If rejected candidates or their online scores are discarded, the offline
`proxy_fidelity` comparison can never be computed.

## Requirement

For every completed online selection decision, Rust storage must accept one
immutable trace containing the selected candidate and every rejected candidate.
Every entry identifies its story artifact, online score, deterministic rank and
selection disposition. The trace also records the policy version used.

## Acceptance criteria

1. A trace has non-empty job, run, decision and policy identifiers.
2. A trace contains at least two candidates and exactly one selected candidate.
3. Candidate IDs and positive ranks are unique within a trace.
4. Scores are finite and ranks are contiguous from one.
5. Every candidate references a non-empty immutable artifact hash.
6. Storage exposes one atomic `put_if_absent` operation for the whole trace.

## Exclusions

- No proxy-fidelity computation in this change.
- No online-weight tuning.
- No SQLite implementation before the runtime persistence slice.
- No chain-of-thought, generator identity or candidate self-confidence.
