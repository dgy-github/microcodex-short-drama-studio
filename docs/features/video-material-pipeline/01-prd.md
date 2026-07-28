# REQ-201 — Video material extraction foundation

Status: deferred; explicitly outside the current story-writing release

> Scope correction, 2026-07-27: the current product writes stories only. It
> neither downloads video nor extracts video material. This document is retained
> as future research and does not authorize implementation or dependency work.

## Problem

The studio can write and evaluate stories, but it does not yet have a trusted
pipeline that turns licensed source video into reusable, traceable material.
Without a common media boundary, the desktop, provider layer and future
sidecars could each start invoking FFmpeg or storing incompatible artifacts.

## Requirement

Given a local video file and an explicit rights declaration, the product must
asynchronously produce a versioned `material-bundle/v1` containing:

- immutable source identity and provenance;
- L0 media validation and normalized probe metadata;
- extracted audio suitable for transcription;
- scene-change keyframes with uniform-sampling fallback;
- optional ASR, OCR and vision-analysis references;
- hashes, tool versions, parameters and parent-artifact links for every output.

Progress and terminal state use the existing runtime event model. The desktop
calls Rust commands only. Provider credentials, process execution, durable
storage, rights checks and budget enforcement remain inside Rust.

## Acceptance criteria

1. Missing rights metadata is rejected before media processing starts.
2. FFmpeg and ffprobe run without a shell, have bounded output, timeout and
   cancellation, and leave no partial artifact after failure.
3. Keyframe extraction uses scene threshold `0.4`, then uniform fallback,
   SHA-256 deduplication and a hard maximum of 60 frames.
4. A resumed consumer can reconstruct job progress from durable events.
5. Re-running the same source with the same toolchain and parameters records
   the same input identity and an explicit derivation record.
6. Svelte, Python and model providers cannot receive unrestricted filesystem or
   process capabilities.

## MVP scope

There is no video-material MVP in the current release. If a future release
reactivates this proposal, its previously considered starting scope was:

- Windows desktop, local files, MP4/MOV/MKV/WebM inputs.
- FFmpeg/ffprobe probe, audio extraction and keyframes.
- Remote ASR and vision calls through `story-provider`.
- SQLite metadata plus content-addressed files managed by Rust.

## Exclusions

- All video download and video material extraction in the current release.
- Douyin scraping, cookies, anti-bot bypass or downloading protected material.
- Timeline editing, rendering and automatic publishing.
- Temporal, OpenCV and a local Whisper runtime.
- Direct FFmpeg, filesystem, database or provider access from Svelte/Python.
