# ADR-0002: Advisory runtime before human calibration

Status: Accepted
Date: 2026-07-28

## Context

P1 completed its engineering work and measured three model families, but none
made the stage-0 instrument both stable and specific. The remaining check is a
blind human review. Meanwhile the product has no end-to-end runtime artifact;
continuing to block all runtime work would keep refining one synthetic pair
without producing the real material a human should evaluate.

## Decision

Defer the internal blind spot check until the first end-to-end story package
exists. Unlock P2.5 and an explicitly advisory P3b runtime prototype.

Prototype outputs carry `advisory/non-promotable` status. They may exercise
contracts, persistence, orchestration and the desktop, but they cannot:

- freeze `eval-v0.1.0` or `judge-v1`;
- promote a model, prompt, graph, retriever, policy or skill;
- satisfy a release acceptance gate;
- be described as validated story quality.

The first end-to-end artifact becomes input to the deferred blind review. P3a
then resolves the rubric decision and freezes the evaluation contract. The
end-to-end run must be repeated against that frozen contract before P3/P5 can
claim release-level completion.

## Consequences

- P2.5 and P3b prototype engineering can proceed now.
- P5 desktop work may follow real P3b command/event contracts.
- P1 remains unresolved rather than passed.
- Human review moves later but remains mandatory before freeze, promotion and
  release.
- Prototype work may require adjustment after the rubric decision; that risk
  is accepted to obtain a real artifact sooner.

## Rejected alternatives

- **Keep all work blocked at P1:** yields no new product artifact or evaluation
  evidence.
- **Declare P1 passed:** contradicts the measured stability and specificity.
- **Drop human review:** violates the project quality contract.
- **Freeze the current rubric provisionally:** would turn an unresolved
  instrument into a false acceptance standard.
