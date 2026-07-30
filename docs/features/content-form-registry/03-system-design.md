# Content-form registry design

Status: G7 complete

## Ownership

`story-core` (`CAP-001`) continues to own the closed `ContentForm` and
`StoryJob` values. `story-runtime` (`CAP-002`) owns the new lookup because the
binding is selected at run preparation. No UI or sidecar layer receives a
second form-to-assets map.

The wire contract is `schemas/content-form-registry-v1.json`; the initial
configuration is `config/content-forms.json`.

## Contract

Each entry binds one `content_form` to:

1. `artifact_schema`;
2. `rubric`;
3. `case_set`.

All asset references use `/`, are relative, contain no empty, `.` or `..`
segments, and cannot be replaced by fields from a job payload. The registry
document has a fixed `content-form-registry/v1` discriminator and rejects
duplicate form entries.

## Failure handling

Parsing and semantic validation happen before lookup. Invalid configuration
returns stable `RegistryError` variants. Lookup of an accepted job form without
a binding fails closed as `UnregisteredForm`; the runtime must not guess a
default triple.

The checked-in case-set reference points at the existing evaluation manifest.
That manifest remains advisory and non-promotable under ADR-0002.
