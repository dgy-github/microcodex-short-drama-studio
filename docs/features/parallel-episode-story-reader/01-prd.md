# Parallel episode writing and story reader

Status: G5 deterministic real-process integration passed

## Requirements

- `REQ-303`: after the fixed workflow approves one episode plan, `t10` acts as
  the parent episode-room coordinator and delegates one generation request per
  episode with a bounded concurrency of three.
- `REQ-304`: every child result is attributed to its episode, retained in the
  single `t10` artifact, and remains subject to the existing reviews,
  targeted revision, final review, package validation, token budget and
  advisory/non-promotable rules.
- `REQ-305`: the desktop artifact browser exposes a full-story reader showing
  the logline, complete character cards, every episode outline and all
  available scripted scenes grouped by episode.
- `REQ-306`: old packages that contain only representative scenes remain
  readable and explicitly identify episodes without scripted scenes.

## Acceptance

- A six-episode deterministic run observes at least two concurrent episode
  generation calls and never more than three.
- The merged `t10` artifact contains one attributed scene group per episode.
- The final package preserves at least one scripted scene for every episode.
- The reader opens from the selected story card, closes with its button,
  backdrop or Escape, and resolves dialogue speaker references to names.
- No provider key or provider endpoint enters Svelte or Python.

## Non-goals

- Changing the top-level 17-task contract, promotion policy, video generation,
  unrestricted agent creation, or unlimited provider concurrency.
