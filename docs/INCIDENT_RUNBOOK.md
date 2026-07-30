# Incident runbook

## First response

1. Stop starting new runs. Do not delete the event database or `.partial` files.
2. Record the application version, run ID, last event ID, and the structured
   diagnostic code. Never paste credentials or raw prompts into an incident.
3. Rotate the affected provider credential in the desktop settings. Confirm a
   new `credential-audit-event/v1` action appears.

## Preserve and recover

1. Close the desktop app so the sidecar event database is quiescent.
2. Run storage repair only against the exact absolute product-data directory.
   Repair removes interrupted `.partial` files and hashes all durable files.
3. Create an integrity backup to a new path. Keep `backup-manifest.json` with
   the backup.
4. Restore only to a new empty path, then point a recovery installation at that
   path. Never overwrite the sole copy.
5. Restart. Accepted non-terminal runs emit `run.recovered`; completed results
   are read from `workflow.result.stored`.

## Escalation

- Hash mismatch: quarantine the backup; do not force restore.
- Repeated `provider_or_task_failure`: disable new runs and inspect provider
  status without logging request content.
- `capability_timeout`: keep the run evidence; connection loss alone is not a
  task failure.
- Rights revocation: disable the referenced retrieval collection and follow the
  adversarial/genre retirement replacement policy.

Close the incident only after story packages, revisions, approvals, and terminal
event counts match the pre-incident manifest.
