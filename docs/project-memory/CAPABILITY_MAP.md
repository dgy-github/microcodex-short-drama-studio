# Capability map

| Capability | Trusted owner | Notes |
| --- | --- | --- |
| Story domain and typed artifacts | `crates/story-core` | Canonical domain types and validation |
| Runtime orchestration | `crates/story-runtime` | Commands, append-only events, task lifecycle |
| Provider access | `crates/story-provider` | Model credentials and provider protocol |
| Durable storage | `crates/story-storage` | Artifacts, approvals, reviews and event durability |
| Policy and budget | `crates/story-policy` | Rights, budget and hard-rule decisions |
| Evaluation | `crates/story-eval`, `eval/` | Offline metrics, cases and judge probes |
| Campaign adapter | `sidecar/campaign_adapter` | Typed sidecar capability boundary |
| Desktop UI | `apps/desktop` | Must call Rust only |
