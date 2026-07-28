# ADR-0001: Rust-owned video material pipeline

Status: Deferred — outside the current story-writing release
Date: 2026-07-27

This ADR records research only. It does not authorize media implementation,
dependency additions or FFmpeg packaging. A future release must reopen the
decision through G0 before using it.

## Context

The product needs video probing, audio extraction, keyframes and optional
ASR/OCR/VLM. Local references offer two useful implementations:

- nanocodex supplies a Rust FFmpeg/ffprobe pipeline and material schemas;
- douyin-downloader supplies robust asynchronous FFmpeg supervision in Python;
- microcodex-short-video-workbench supplies a Tauri 2 + Svelte 5 desktop shell.

The studio architecture already assigns trusted storage, provider access,
rights, budget and process execution to Rust.

## Decision

Build the media pipeline as a Rust-owned capability integrated with
`story-runtime`, Rust storage and `story-provider`. Use pinned FFmpeg/ffprobe
sidecars for codec work and Tauri 2 + Svelte 5 for the desktop surface.

Reuse extraction algorithms and failure-handling behavior, not whole reference
modules. Long-running processes and network requests are asynchronous. Python
receives typed capabilities only.

Start with licensed local files. Treat platform downloading as a later adapter
with separate rights, credential and compatibility review.

## Consequences

- One trusted boundary owns credentials, processes, artifacts and provenance.
- FFmpeg provides broad codec support without adding OpenCV or native media
  bindings to the first release.
- Packaging must ship, hash, license and update platform-specific binaries.
- Provider calls remain swappable and budget-controlled.
- A future local ASR engine can implement the same provider interface without
  changing material contracts.

## Rejected alternatives

- **Python-owned extraction:** conflicts with the trusted-process boundary and
  creates a second storage/runtime owner.
- **Copy nanocodex unchanged:** retains blocking calls and duplicate database
  ownership.
- **Temporal orchestration now:** duplicates the existing runtime.
- **Browser-side extraction:** weakens filesystem, secret and process control.
- **Platform downloader in MVP:** expands legal, cookie and anti-bot scope
  before the local pipeline has an acceptance baseline.
