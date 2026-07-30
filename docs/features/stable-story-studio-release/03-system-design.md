# Stable story-studio release system design

Status: G6 local acceptance complete; G7 clean-machine evidence open

PyInstaller produces a `story-sidecar/` onedir bundle; Tauri includes it as a
resource in both MSI and NSIS targets. Onedir mode keeps the launched process
as the process Rust owns, avoiding an orphaned onefile extraction child.
The build adds the repository `schemas/` directory as frozen data and rejects
the sidecar when `story-package-v1.json` is absent. Frozen Python resolves the
schema below `sys._MEIPASS`; source execution continues to resolve repository
schemas.
The resource directory retains a tracked README placeholder so ordinary Tauri
builds work before packaging; generated sidecar files remain ignored and the
sidecar build cleans only generated children.
Runtime discovery prefers an explicit absolute
`MICROCODEX_SIDECAR_EXE`, then packaged locations, and otherwise falls back to
the development virtual environment. Bundled mode stores its SQLite event log
under per-user local application data rather than the installation directory.

The release script installs from lockfiles, builds the sidecar and desktop,
optionally signs each installer by certificate thumbprint, verifies signatures,
and records artifact SHA-256 plus toolchain and commit identity. The evidence
keeps unsigned output ineligible for a future public trusted release, while the
owner explicitly accepts it for this personal distribution. A normal release
refuses a dirty worktree; local pipeline verification must opt in and records both
`dirty=true` and a source-state SHA-256 over the tracked binary diff plus every
untracked non-ignored file's path, size and SHA-256.

Before signing or emitting evidence, the release pipeline administratively
extracts the generated MSI into a bounded directory under `target/`. It runs
the real Rust duplicate-Start/`Last-Event-ID`/idempotent-Cancel test against the
extracted sidecar, rejects any leaked sidecar process, and requires the
extracted desktop executable to stay alive for five seconds. Evidence records
all results and the extracted desktop, sidecar, WebView2 loader, and story
schema hashes.
GNU Rust dynamically links `WebView2Loader.dll`; WiX includes it automatically
and an explicit NSIS install/uninstall hook places it beside the desktop
executable. The release build rejects either installer source when that DLL is
absent. The temporary extraction is removed in a `finally` path.

Before building the sidecar or installers, the pipeline regenerates the
lockfile-backed dependency inventory. Any `UNKNOWN` license fails a normal
release. `-AllowUnlicensedForLocalVerification` is accepted only together with
the explicit local/dirty-build switch and without a certificate thumbprint;
evidence records `distribution_license.cleared=false`, the unresolved names,
and `installer_release_eligible=false`. That field retains conservative public
release semantics; it is not the acceptance signal for the owner-approved
unsigned personal distribution.

`config/distribution-license-policy-v1.json` is the deterministic path for
clearing metadata that is unavailable on a clean runner. An approval must match
the exact pinned source revision and bind authoritative license text under
`third_party/licenses/` by SHA-256, reviewer and timezone-qualified review time.
The inventory hashes the policy into release evidence. Unapproved entries must
remain `UNKNOWN` with null evidence/reviewer fields.

The owner selected MIT for `campaign-muti-agent`. The policy now pins revision
`1d935714449d18cad5bdc6711a498297ed73a5fb`, retains its authoritative
`LICENSE`, and verifies the retained bytes before admitting distribution.
The sidecar build also force-reinstalls that exact PEP 508 requirement and
fails unless installed `direct_url.json` proves the expected source URL,
requested revision, resolved commit and MIT package metadata.

The `windows-release-smoke` manual CI job runs the same pipeline on a clean
`windows-2022` runner with exact pinned toolchains. It does not upload unsigned
installers. A configured local run proves the workflow implementation; G7 still
requires an externally observed clean-runner result, while organization signing
is optional.

When the manual job requests signing, a wrapper reads the base64 PFX and
password only from GitHub encrypted secrets, writes the PFX under
`RUNNER_TEMP`, imports exactly one currently valid code-signing certificate
with a private key into `CurrentUser\My`, and invokes the same release build.
Its `finally` path removes both the exact imported thumbprint and the temporary
PFX and clears the secret environment variables. The release build discovers
the newest x64 Windows SDK `signtool.exe` when it is not on `PATH`, timestamps
over HTTPS, verifies every installer with `/pa`, and records the public
certificate thumbprint and verification result. CI uploads evidence only, not
the unsigned installers.

Before store import, the wrapper parses the PFX with `EphemeralKeySet`, requires
exactly one private-key certificate, and rejects a thumbprint already present
in `CurrentUser\My`; cleanup therefore cannot remove a pre-existing identity.

The application semantic version is `0.1.0-alpha.1`. WiX rejects textual
prerelease identifiers, so Windows packages use the separately recorded numeric
installer version `0.1.0-1`; release configuration tests bind the two values.

Rust `1.88.0`, Node `22.14.0`, and Python `3.12.10` are repository-pinned.
Rust 1.88 is the minimum supported by the locked Tauri transitive dependency
set; older toolchains are rejected by the release script.
