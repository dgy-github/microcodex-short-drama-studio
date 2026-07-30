# Story domain breadth

Status: G6 non-human acceptance complete; hidden promotion excluded

## Requirements

- `REQ-132`: register versioned genre packs without changing Rust, sidecar, or
  evaluation arithmetic.
- `REQ-133`: bind short and long episode-count profiles to the same
  `story-job/v1` and `story-package/v1`.
- `REQ-134`: bind genre-specific architect and reviewer profiles.
- `REQ-135`: allow retrieval only from collections with per-source content
  hashes and rights evidence.
- `REQ-136`: require genre-matching regression cases for every pack.
- `REQ-137`: refresh challenge cases every 92 days from rights-cleared
  production failures.
- `REQ-138`: retire leaked, rights-revoked, invalid, duplicate, or saturated
  adversarial pairs only under an explicit replacement rule.
- `REQ-307`: expose eight draft genre packs covering family, suspense, urban
  romance, revenge, workplace, rural, comedy, and historical stories; every
  added pack has genre-matching agent profiles and regression cases.
- `REQ-308`: the desktop genre selector is projected from the Rust-owned
  registry instead of duplicating pack IDs, labels, genres, or audiences in
  Svelte.

All registered packs are draft candidates. None claims hidden-gate promotion.

Rust resolves the selected pack and constraints before provider access. The
typed context is forwarded to the sidecar, where architect/reviewer directives
and rights-cleared retrieval provenance are consumed by the fixed workflow.
