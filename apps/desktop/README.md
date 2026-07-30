# Desktop App Boundary

The Tauri 2 + Svelte 5 application is the Windows presentation boundary.

The frontend will consume typed Tauri events only. It must never connect to the
Campaign Python sidecar or model providers directly.

The P5 desktop validates story jobs, configures provider credentials through
Rust, starts/cancels/resumes runs with durable event projection, browses
completed advisory artifacts, and operates immutable revisions and approvals.

## Development

```powershell
cd apps\desktop
npm install
npm run check
npm run build
npm run tauri -- dev --no-watch
```

The Rust shell is an independent workspace at `src-tauri` so the Tauri
dependency graph does not enter the product's root Rust workspace.

## Windows release verification

From the repository root, `scripts\build_windows_release.ps1` builds the onedir
sidecar and MSI/NSIS installers. Every build administratively extracts its MSI,
runs the real duplicate-Start/`Last-Event-ID`/idempotent-Cancel test against the
packaged sidecar, checks the desktop launch, and records the results in
`target\release-evidence\windows-release-evidence.json`. A normal release
requires a clean worktree; `-AllowDirty` produces local verification evidence
only.

A normal build fails before packaging while any dependency license is
`UNKNOWN`. Until Campaign licensing is resolved, local unsigned pipeline
verification must explicitly use both `-AllowDirty` and
`-AllowUnlicensedForLocalVerification`; evidence records the unresolved
dependency and `installer_release_eligible=false`. That override cannot be
combined with signing.

The manual `windows-release-smoke` workflow can sign when
`WINDOWS_SIGNING_PFX_BASE64` and `WINDOWS_SIGNING_PFX_PASSWORD` repository
secrets are configured. `scripts\build_signed_windows_release.ps1` imports the
certificate only for the build and removes the exact certificate and temporary
PFX in a `finally` path. CI uploads JSON evidence only; it does not upload the
installers while Campaign distribution licensing is unresolved.
