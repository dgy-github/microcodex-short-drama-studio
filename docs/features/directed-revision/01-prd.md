# Directed revision and approval

Status: G6 implementation and desktop integration passed

## Requirements

- `REQ-120`: import one generated `story-package/v1` as an immutable origin.
- `REQ-121`: navigate from a review finding's `ArtifactSpanRef` to the cited
  node and create a schema-valid replacement revision.
- `REQ-122`: permit at most two targeted D3/D4 rounds. Exceeding the bound
  returns `input_required`; it never silently spends another provider round.
- `REQ-123`: populate complete `node_correspondence` whenever a package
  supersedes another package.
- `REQ-124`: record approval or rejection as a separate append-only event.
- `REQ-125`: compare immutable revisions and implement rollback as a new
  revision that supersedes the current package.
- `REQ-126`: export only an explicitly approved, schema-valid package.

## Acceptance

- Existing revision and package files are never overwritten.
- A targeted replacement preserves the cited node identity.
- Correspondence identifies unchanged, changed, and removed prior nodes.
- Exported bytes validate against `story-package/v1`.
- Desktop commands return projections, never direct filesystem ownership.

## Non-goals

- No promotion, human blind evaluation, video asset, or unrestricted JSON-path
  mutation.

## Evidence

- `story-storage` tests cover targeted correspondence, the two-round D3/D4
  bound, approval immutability, rollback-by-new-revision, and approved export.
- `story-policy` tests cover D3 cited-defect priority and D4
  revise/complete/input-required decisions.
- Desktop service integration imports a real baseline package, navigates a
  cited span, and creates a revision.
- Svelte check and production build pass with the revision workspace connected.
