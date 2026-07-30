# Desktop operator guide

## First run

1. Install the MSI or NSIS package. The current personal distribution is
   unsigned, so Windows may show an unknown-publisher or SmartScreen warning.
2. The app opens **模型配置** when either required credential is absent.
3. Review the HTTPS Chat Completions endpoint and model ID shown for DeepSeek
   and Alibaba Cloud Bailian. Edit and save either route when the provider URL
   or deployed model changes.
4. Save a DeepSeek key for generation and an Alibaba Cloud Bailian key for
   review. Existing keys are replaced as a rotation and recorded in the local
   credential audit.
5. Run **健康检查** for both providers. It performs one minimal structured
   request and reports the model identity without exposing the key.
6. Before release evidence is collected, run **双供应商稳定性检查**. This is a
   deliberate paid operation; the selected iteration count is executed once
   against each provider and only timing/count evidence is retained.
7. Open **创作台**, choose a genre pack and short/long constraint, validate the
   job, then start the fixed 17-task run.
6. Follow progress in the run console. Reconnecting resumes after the last
   durable event; connection loss is not a task failure.
7. Open **作品库** to review artifacts, navigate findings, create up to two
   targeted revisions, approve, compare, roll back by new revision, and export
   only an approved revision.

## Accessibility and language

The application declares `zh-CN`, uses native labels for all form controls,
semantic headings/navigation, keyboard-focusable native controls, text status
alongside color, and a responsive minimum window. Windows text scaling and
keyboard navigation are supported. The first stable contract ships Simplified
Chinese UI copy; user data and story contracts remain UTF-8 and locale-neutral.

## Recovery

Do not delete product data after a crash. Restarting resumes accepted
non-terminal work from the durable command. Follow `INCIDENT_RUNBOOK.md` for
repair, backup, and restore. Never restore over the only copy.
