# Story domain breadth system design

Status: G6 non-human acceptance complete; hidden promotion excluded

The genre registry is entirely configuration-driven. Packs reference versioned
constraint, agent, retrieval, and regression documents by stable ID. The
validator resolves every reference, verifies source hashes and rights evidence,
and rejects any promotion claim without an evaluation run.

Both episode-count variants retain `content_form=scripted_short_drama`.
`GenrePackRegistry` resolves configuration to a typed `GenreContext`; the same
StartRun and fixed 17-task workflow consumes either pack without a pack-specific
runtime branch, storage owner, artifact schema, or scoring arithmetic.

`GenrePackRegistry::options` projects only pack ID, display name, genre, and
default audience through desktop IPC. Svelte renders that projection and sends
the selected stable IDs back in `story-job/v1`; it does not own a parallel
genre map. A missing or invalid registry fails closed before a story starts.

Challenge maintenance is a read-only planning tool. It marks quarterly refresh
readiness and retirement candidates, but replacement-required policy prevents
coverage from disappearing through automatic deletion.
