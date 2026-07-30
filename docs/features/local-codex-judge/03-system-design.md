# Local Codex judge system design

Status: G7 complete

`eval/tools/run_stage0_probe.py` owns this offline integration. The adapter
launches `codex exec` without a shell, uses stdin for the complete blinded
prompt, writes the final response to a temporary file, and parses JSONL only
for usage metadata.

The process working directory is a fresh temporary directory outside the
repository. `--skip-git-repo-check`, `--ephemeral`, `--sandbox read-only`, an
explicit model, and a tracked output schema bound the execution. No workspace
directory is added.

When an older standalone CLI reads a newer desktop `models_cache.json`, the
schemas can differ. The adapter never edits or deletes the shared cache. It
creates a temporary static catalog and supplies the missing
`supports_reasoning_summaries=false` capability because this judge does not
request reasoning summaries. Reasoning levels newer than the installed CLI
understands are omitted from that temporary copy; the configured default
`low` level is retained.

The base input fingerprint remains unchanged so the existing remote samples do
not become stale merely because a supplemental route was added. A second
`judge_config_fingerprint` binds Codex results and checkpoints to the exact
supplemental judge object.

Failures are hard failures for that judge. The adapter does not fall back to a
different model, provider, or route within a probe.
