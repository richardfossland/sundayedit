# CLAUDE.md — Verbatim

Verbatim is a desktop AI captioning application. Mac and Windows, equal first-class support. Standalone product, own brand. Optional integration with Sunday Account.

## Target user

A content creator, video editor, podcaster, educator, journalist, marketer, or church volunteer who:

- Records or edits video with spoken content
- Currently uses Premiere Pro, Descript, CapCut, or Veed.io for captions
- Is frustrated by either price (Adobe), bloat (Descript), or quality (free tools)
- Wants control: desktop performance, offline capability, no required cloud upload

## Core promises

1. **Faster than any competitor from raw video to broadcast-ready captions** — because the AI does 92% of the work and tells you exactly where to look for the 8% it's unsure about.
2. **Context-aware:** tell Verbatim what the video is about, and accuracy on names, jargon, foreign words goes way up.
3. **Privacy-first:** local Whisper by default. Cloud transcription is optional, off by default, requires explicit consent.
4. **Affordable:** 1/3 the price of Premiere Pro Creative Cloud.
5. **Polished export:** SRT, VTT, ASS, plus burn-in to MP4 with full styling.

## The two genuine innovations

### #1 — Confidence highlighting

Every word gets a confidence score from the ASR model. Verbatim shows them to you as colour-coded highlights in the editor. The 92% the AI is sure about, you don't touch. You fix only the 8% that are amber. **This makes human review 10× faster.**

### #2 — Context priming + glossary

Before transcription you tell Verbatim what the video is about: names of people, technical terms, foreign words. Whisper biases recognition toward these terms. The same video goes from "Han snakker om kerigma og frelse" to "Han snakker om kerygma og frelse" without manual correction.

## Competitive positioning

- vs **Premiere Pro:** focused product, 1/3 the price, AI-native from the start
- vs **Descript:** not a "doc pretending to be a video editor" — we are a captioning tool, no scope creep
- vs **CapCut:** professional output for professional work; we are not TikTok-first
- vs **Veed.io / Kapwing:** desktop-native performance, no upload required, works offline

## Tech principles

- **Local-first.** Cloud features are opt-in.
- **The video file never leaves the user's machine** unless they explicitly request cloud transcription.
- **AI is in service of human judgment** — we surface uncertainty, we don't hide it.
- **The editor must feel instant.** No spinners during normal editing.
- **Output quality is non-negotiable** — what gets exported must match what the user saw in preview, pixel-perfect.

## Stack

- **Tauri 2** (Rust backend) + React 19 + TypeScript + Tailwind v4
- **whisper-rs** (whisper.cpp Rust bindings) for local speech recognition
- **ffmpeg** sidecar binary for video I/O + burn-in rendering
- **SQLite** via `sqlx` for project files
- **shadcn/ui** primitives, customized — dark-first, professional (think Linear, Final Cut)
- **TanStack Query** (server state) + **Zustand** (UI state)

## Out of scope for v1

- Full video editing (cuts, transitions, effects beyond captions)
- Multi-track audio mixing
- Color grading
- Anything that isn't directly about captions

## Repository layout

```
src/                  React frontend
├── app/              Route/page-level components
├── features/         Feature modules (project, transcribe, editor, style, export)
├── components/       Shared UI primitives
├── lib/              Utilities, hooks, IPC client, types
└── styles/           Globals, design tokens

src-tauri/            Rust backend
└── src/
    ├── commands/     Tauri command handlers
    ├── services/     Business logic (caption operations, export, ASR — later)
    ├── db/           Recent files + project metadata
    ├── error.rs
    └── lib.rs

sql/                  Migration files
docs/                 Architecture, decisions, calibration data
tests/fixtures/       Test video files (Creative Commons)
```

## Project file format

`.verbatim` files are SQLite databases with a JSON-compatible schema, or compressed JSON — final decision in Phase 3.1. Containing:

- Reference to source video (absolute path + content hash for path stability)
- Project settings
- Caption data (with per-word timing + confidence)
- Style data
- Context + glossary
- History (undo stack)

## Calibration discipline

Confidence highlighting is the killer feature. It MUST be calibrated empirically:

- Take 10 real transcripts, label every word manually as correct/incorrect
- Plot precision/recall against confidence thresholds
- Choose tier boundaries based on real data
- Document in `docs/CALIBRATION.md` so reviewers (and us) trust the numbers
