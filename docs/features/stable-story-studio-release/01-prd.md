# Stable story-studio release

Status: G6 local acceptance complete; G7 clean-machine evidence open

## Requirements

- `REQ-145`: package the Svelte/Tauri desktop and Python sidecar for Windows
  MSI/NSIS installation without a machine Python dependency; every release
  build must extract its MSI and pass the bundled sidecar protocol and desktop
  launch smokes. The sidecar build must force-install and verify the exact
  pinned Campaign source revision even when an older commit has the same
  package version, and must include every runtime story schema required by the
  frozen 17-task workflow.
- `REQ-146`: optionally sign and verify every installer and emit hash/toolchain
  release evidence. CI signing credentials must come only from encrypted
  secrets, use a code-signing certificate with a private key, and be removed
  from the runner certificate store and filesystem after the build.
- `REQ-147`: route first-run users to credentials and provide live provider
  health diagnostics.
- `REQ-148`: document accessibility, Simplified Chinese locale, operation,
  incident response, upgrade, and rollback.
- `REQ-149`: declare stable contract compatibility and release notes.
- `REQ-150`: fail before packaging when any bundled dependency lacks explicit
  distribution-license metadata. A local unsigned verification override must
  be explicit, cannot be combined with signing, and must emit
  `installer_release_eligible=false` plus the unresolved dependency names.

The Campaign distribution-license blocker is closed with owner-selected MIT
evidence. The owner explicitly accepts unsigned installers for this personal
project, so package signing is optional and not a phase exit condition. Clean
Windows release evidence remains required.
