# REQ-302 — Form-agnostic artifact span addressing

Status: approved for P2.5/P4 implementation

## Requirement

A defect or revision target may cite a stable artifact location without knowing
the artifact's content form. The canonical representation is:

```text
<artifact-kind>/<node-kind>-<positive-index>(/<node-kind>-<positive-index>)*
```

Examples include `story-package/scene-2/dialogue-7` and
`story-package/character-1`.

Artifact and node kinds use lowercase ASCII letters, digits and hyphens and
start with a letter. A node segment ends in a positive decimal index.

## Acceptance criteria

- Rust owns one validated `ArtifactSpanRef` type.
- Invalid, blank, zero-indexed or root-only references are rejected.
- `story-policy::Defect` can optionally cite this type.
- A missing span remains valid for artifact-wide defects.

## Exclusions

- Resolving a span against artifact bytes.
- Node-correspondence traversal across revisions.
- UI navigation.
