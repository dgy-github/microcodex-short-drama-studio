# System design

`CAP-004` remains the workflow owner. `t01` through `t09` establish the genre,
architecture, characters, causal beats and immutable episode plan. `t10` is
the parent episode-room coordinator. It creates one typed generation request
per planned episode, supplies the shared character/beat context plus only the
target episode, and executes those requests through an `asyncio.Semaphore(3)`.

Each child lane emits durable `episode.started`, `episode.completed` or
`episode.failed` events using `episode-writer-NN` as the agent identity. The
parent aggregates usage before the existing token-budget retention boundary.
Any child failure fails `t10`; there is no partial successful story package.

The merged `sample-scenes/v1` remains an internal task artifact, so the public
17-task workflow-result contract does not change. Each scene carries an
internal `episode_index` until `canonical_package` converts it to a validated
`episode_ref`. `t11` through `t16` review and revise the merged material, while
`t17` keeps the existing Rust-owned package validation.

`CAP-008` continues to read the immutable `story-workflow-result/v1` through
Tauri. Svelte receives no new backend authority. The reader is a presentation
of the already-returned package and groups `scenes` by `episode_ref`. Packages
created before this change degrade to an explicit “outline only” episode.

Failure remains closed. The concurrency bound is fixed by product code, child
usage is summed, missing episode output is rejected, and the final package must
contain at least one scripted scene for every requested episode.
