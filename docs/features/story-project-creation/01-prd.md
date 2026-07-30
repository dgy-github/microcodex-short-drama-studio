# REQ-104 — Story project content form

Status: G7 contract slice complete

## Requirement

The customer-facing story job used to create a project must carry a required,
immutable `content_form`. The first release accepts exactly
`scripted_short_drama`, matching the existing genre templates.

Given a serialized `story-job/v1`:

- omission of `content_form` is invalid;
- any unregistered form is invalid;
- Rust exposes the selected form for routing but no mutation API;
- the form remains part of the job when serialized again.

This field selects the artifact-schema, rubric and case-set family. It does not
select a genre template.

## Exclusions

- No explainer or real-creator form is added.
- No cross-form conversion is defined.
- No desktop UI or runtime command is added before P3b contracts exist.
- Existing evaluation artifacts are not re-scored.

## Acceptance

Rust and JSON Schema agree on the one accepted value. Tests reject both a
missing form and an unknown form, and round-trip the accepted form.
