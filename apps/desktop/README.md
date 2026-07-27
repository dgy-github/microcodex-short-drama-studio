# Desktop App Boundary

The Tauri 2 + Svelte 5 application will be scaffolded after the M0 command,
event, SSE, and artifact contracts pass their integration tests.

The frontend will consume typed Tauri events only. It must never connect to the
Campaign Python sidecar or model providers directly.

