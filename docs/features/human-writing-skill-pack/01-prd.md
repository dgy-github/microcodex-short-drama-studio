# Short-drama human writing skill pack

Status: G2 contracts ready

## Requirements

- `REQ-312`: every configured genre pack resolves one project-owned,
  versioned human-writing profile without loading third-party text.
- `REQ-313`: the profile injects task-specific directives into character
  design (`t07`), episode writing (`t10`), human-taste review (`t12`),
  targeted revision (`t15`) and final review (`t16`).
- `REQ-314`: human-writing review remains evidence-based. It reports cited
  defects and never promotes an artifact by itself.

## Acceptance

- Rust rejects a missing, malformed or unknown human-writing profile.
- The sidecar injects only the directives assigned to the current task.
- Episode child agents receive the same `t10` directives.
- Deterministic workflow tests observe all five injection points.

## Exclusions

- No third-party prompts, prose examples, word lists or genre corpora.
- No mechanical synonym replacement, intentional grammar damage or claim
  that generated prose is human-authored.
- No additional workflow task or provider call.
