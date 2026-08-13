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

## Tests

```powershell
npm test -- --run            # Vitest component and unit tests
npm run test:e2e:tauri       # WebdriverIO acceptance against the real binary
```

`test:e2e:tauri` builds the frontend, then compiles the shell with the
`custom-protocol` feature into `src-tauri/target-e2e` and drives the resulting
executable through real Tauri IPC. The feature matters: without it `cargo build`
produces a development binary that loads `tauri.conf.json`'s `devUrl`, so the
window shows `ERR_CONNECTION_REFUSED` unless a Vite dev server happens to be
running.

The acceptance run launches the Python sidecar, which resolves to
`.venv/Scripts/python.exe` at the repository root unless `MICROCODEX_PYTHON`
overrides it. Create that environment first:

```powershell
python -m venv .venv
.venv\Scripts\python -m pip install --editable sidecar
```

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
