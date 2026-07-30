# Directed revision system design

Status: G6 implementation and desktop integration passed

`story-storage` extends its existing immutable artifact responsibility with a
filesystem-backed `RevisionRepository`. `story-core::ArtifactSpanRef` remains
the only accepted address type. The repository validates every package against
the tracked `story-package/v1` schema before a durable write.

Each revision directory contains an immutable record and package. Approval is a
separate create-once event. A targeted revision replaces one complete
addressable node while preserving its `node_id`; this avoids ambiguous field
paths and keeps defect citations stable.

The repository derives correspondence by enumerating addressable nodes before
and after revision. Rollback copies a prior package into a newly identified
package, sets `supersedes` to the current package, and computes correspondence
from current to new. Export requires an approved event and uses a temporary
file plus rename.

Svelte consumes typed Tauri commands for open, navigate, revise, approve,
compare, rollback, and export. It never validates packages or writes history.
