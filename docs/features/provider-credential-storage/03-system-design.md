# Provider credential storage design

Status: G7 complete

## Ownership and trust boundary

This extends `story-provider` (`CAP-003`, `IFACE-003`), which already owns
provider credentials and calls. No UI, sidecar or storage-crate API accepts
secret bytes.

`ProviderCredentialId` validates provider and profile components before they
become a Credential Manager target. The fixed service is
`com.microcodex.short-drama-studio.provider`; the account is
`<provider>/<profile>`.

`ProviderSecret` is not cloneable, prints only `[REDACTED]`, and zeroizes its
owned bytes on drop. Callers receive a short-lived borrow through
`expose_secret`.

## Windows backend

The Windows implementation uses `keyring 3.6.3` with only the
`windows-native` feature. Version 3 is selected because its Rust 1.75 MSRV fits
the workspace's Rust 1.85 contract; the current version 4 Windows backend
requires a newer compiler.

All operations on one store instance are serialized. Windows Credential
Manager does not guarantee ordering for concurrent accesses to the same
credential.

Backend error details are collapsed into stable product errors. Platform error
strings are not propagated because they are not a public contract and may
contain machine-specific information.

## Failure handling

- Missing get: `Ok(None)`.
- Missing delete: `Ok(false)`.
- Empty secret or malformed identity: rejected before platform I/O.
- Locked/unavailable store: `CredentialStoreError::Unavailable`.
- Ambiguous or malformed stored entry: stable non-secret error variants.
