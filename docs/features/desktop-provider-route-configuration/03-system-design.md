# Desktop provider route configuration design

Status: G6 implementation and local integration passed

`story-provider` remains the route-validation owner. The desktop adapter stores
only a `desktop-provider-route/v1` projection under
`%LOCALAPPDATA%/MicrocodeX/ShortDramaStudio/provider-routes`.

Each save uses a generated immutable record and a temporary sibling followed by
rename. Loading ignores temporary files, validates every candidate, and selects
the greatest `(updated_at_unix_ms, record_id)` tuple. This avoids overwriting a
last-known-good route and makes interrupted writes recoverable.

The desktop service resolves defaults when no record exists. DeepSeek and
Aliyun Bailian keep provider-specific `thinking_disabled` policy in Rust; users
edit only endpoint and model. Credential bytes are loaded separately from
Windows Credential Manager when Rust constructs a `ProviderRoute`.

The health command, `DesktopRunController`, and `EvaluationService` receive the
same settings service. No consumer owns fallback route constants.
