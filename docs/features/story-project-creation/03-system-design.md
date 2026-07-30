# Story project creation contract

Status: G7 contract slice complete

## Ownership

`story-core` remains the owner of customer job values (`CAP-001`,
`IFACE-001`). `schemas/story-job-v1.json` is the wire contract. No new owner is
created.

`ContentForm` is a closed Rust enum serialized as snake case. `StoryJob` keeps
the field private and exposes a read-only getter. The JSON contract requires
the field, so legacy payloads fail explicitly instead of silently selecting a
form.

## Compatibility

No production job store or runtime exists, so there are no persisted jobs to
migrate. This is a pre-runtime correction to the alpha `story-job/v1`
contract. The canonical value is `scripted_short_drama`, already used by all
tracked genre templates.

## Deferred integration

The Tauri create-project command and Svelte form wait for P3b's command,
storage and event contracts. When added, they must deserialize this exact Rust
type and may not duplicate the form list in frontend-only code.
