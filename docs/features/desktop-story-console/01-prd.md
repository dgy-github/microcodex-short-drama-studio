# Desktop story console

Status: G6 implementation and deterministic desktop E2E passed

## Requirements

- `REQ-115`: a Windows user can compose and validate one
  `story-job/v1` without a developer console. The form fixes
  `content_form=scripted_short_drama` and exposes premise, genres, audience,
  episode format, limits, token/cost budget, and deadline.
- `REQ-116`: a Windows user can save, inspect presence of, replace, and delete
  DeepSeek or Qwen credentials through Tauri commands backed by the existing
  Rust credential owner. Secret bytes are never returned to Svelte.
- `REQ-117`: a Windows user can list completed advisory runs and inspect their
  package and review records through Rust-owned artifact reads.
- `REQ-118`: a Windows user can start one fixed 17-task advisory run through
  Rust, incrementally resume its durable event projection after
  `Last-Event-ID`, and see task, review, approval, error, and token-budget
  state without Svelte learning provider or sidecar addresses.
- `REQ-119`: a Windows user can cancel an accepted or running run. Cancellation
  is idempotent and produces one durable terminal `run.cancelled` event.
- `REQ-311`: the story form defaults a six-episode paid workflow to 180,000
  tokens and shows an episode-scaled recommendation. Users may deliberately
  choose a lower hard limit; the workflow continues to fail closed at that
  limit.

## First vertical slice

The first slice created the Tauri 2 + Svelte 5 shell. The second slice extends
the existing runtime owner with cancellation and projects incremental durable
events through typed Tauri commands.

## Acceptance

- The desktop frontend builds without direct provider or sidecar URLs.
- Rust rejects malformed jobs, unsafe credential identities, unsafe run IDs,
  missing artifacts, and invalid workflow-result shapes.
- Credential status contains only provider/profile/configured fields.
- Artifact reads stay under the configured advisory artifact root.
- Repeated event sync accepts only events after the snapshot cursor and
  deduplicates by sequence.
- Completed workflow results are schema-checked before entering the artifact
  repository.
- Currency consumption remains unknown until provider pricing is configured;
  the UI must not infer it from token counts.
- Rust unit tests and a frontend production build pass.

## Non-goals

- No video workflow, provider request from Svelte, Python connection from
  Svelte, credential echo, artifact mutation, promotion, or quality claim.

## G5 evidence

- Svelte check: zero errors and zero warnings.
- Vite production build: passed.
- Desktop Rust: four tests passed; clippy and debug executable build passed.
- Real Tauri window: `MicrocodeX 短剧工作室`, Responding.
- Startup IPC projected two completed advisory runs from the local artifact
  repository.
- Authenticated process integration passed for duplicate StartRun,
  `Last-Event-ID` replay, first CancelRun, and repeated CancelRun.
- A non-paid desktop E2E starts the real Python sidecar, crosses the
  authenticated capability boundary, rejects a duplicate Start, completes all
  17 tasks and 5 reviews, performs both package validations, and persists the
  advisory/non-promotable workflow result through `ArtifactRepository`.
- Desktop Rust tests, Clippy, Svelte check, and Vite production build pass with
  the live run console connected.

## Unverified

- The paid desktop controller E2E remains ignored until rotated DeepSeek/Qwen
  credentials are configured through the desktop Credential Manager form.
