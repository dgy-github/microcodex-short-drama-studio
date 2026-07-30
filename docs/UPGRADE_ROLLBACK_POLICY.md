# Upgrade and rollback compatibility policy

## Compatibility

- `1.x` readers accept every stored `*/v1` contract produced by an earlier
  `1.x` release.
- Additive optional fields require a minor release. Required fields or changed
  meaning require a new contract major version and an explicit migrator.
- Store migration is forward-only and idempotent. A newer unknown store version
  fails closed.
- Story, revision, approval, event, backup, and release evidence are never
  rewritten in place to imitate an older schema.

## Upgrade

1. Close active runs and create a verified backup to a new path.
2. Verify the installer signature and published SHA-256.
3. Install the newer package. Startup runs only registered idempotent migrations.
4. Run both provider health checks and open one existing approved revision.

## Rollback

Application binaries may roll back only while the prior version declares the
current store version readable. Otherwise restore the pre-upgrade backup to a
new data root and install the prior signed package. Never downgrade the store
version marker or overwrite post-upgrade evidence.

Rollback is complete only after artifact hashes, approval records, and terminal
event counts match the selected backup manifest.
