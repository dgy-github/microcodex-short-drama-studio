# Desktop provider route configuration

Status: G6 implementation and local integration passed

## Requirements

- `REQ-155`: the desktop displays and edits the HTTPS OpenAI-compatible
  endpoint and model ID for DeepSeek and Aliyun Bailian.
- `REQ-156`: Rust persists non-secret route settings below the application data
  root without placing provider credentials in the settings record.
- `REQ-157`: provider health checks, the fixed story workflow, and automatic
  evaluation resolve the same persisted route instead of carrying independent
  hard-coded endpoint or model values.
- `REQ-158`: invalid providers, profiles, non-HTTPS endpoints, non
  `/chat/completions` routes, oversized values, and blank model IDs fail before
  storage or network access.

## Acceptance

- The existing provider defaults are shown on first run and remain editable.
- Saving a route creates an immutable record; interrupted `.partial` files are
  ignored and the latest valid record wins.
- The response contains endpoint/model metadata but never credential bytes.
- Svelte calls typed Tauri commands only.
- Rust tests cover defaults, validation, persistence, latest-record selection,
  partial-file recovery, and the public JSON Schema.

## Evidence

- `ProviderRoute::validate` rejects insecure, credential-bearing, queried,
  malformed, oversized, and blank route values.
- Desktop tests validate defaults, immutable updates, interrupted-write
  recovery, and the public schema.
- Health, story runtime, and automatic evaluation have no independent provider
  URL or model constants.
- Svelte check and production build pass with editable endpoint/model fields.

## Non-goals

- No arbitrary HTTP headers, proxy configuration, provider plug-in system,
  secret storage outside Credential Manager, or direct Svelte/Python provider
  call.
