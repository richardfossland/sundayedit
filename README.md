# SundayEdit

AI-assisted video captioning for desktop. Standalone product — own brand. Optional (never required) integration with [SundayRec](https://github.com/richardfossland/sundayrec).

> ⚠️ **Status:** Phase 0 scaffold + Phase 3.1 (caption model + operations) + Phase 6.1 (SRT/VTT/ASS/TXT export) complete. 49 Rust unit tests green. Video import, Whisper pipeline, styling editor, and burn-in render all pending.

## Two genuine innovations

### #1 — Confidence highlighting

Every word gets a confidence score from the ASR model. SundayEdit shows them as colour-coded highlights. The 92% the AI is sure about, you don't touch. You fix only the 8% that are amber. **Human review at 10× speed.**

The editor demo (`npm run tauri dev` → Editor tab) shows this against sample data: high-confidence words look like normal text, while "kerigma" (38% — a misheard theological term) lights up red-orange with `kerygma` as a one-click alternate.

### #2 — Context priming + glossary

Tell SundayEdit what the video is about before transcribing. Whisper biases recognition toward your names, jargon, and foreign words. "Han snakker om kerigma" becomes "Han snakker om kerygma" with no manual correction.

## Competitive positioning

|                         | Premiere Pro | Descript    | CapCut       | **SundayEdit**          |
| ----------------------- | ------------ | ----------- | ------------ | ----------------------- |
| Price                   | $23/mo       | $24/mo      | Free-ish     | **~$9/mo Pro**          |
| Focus                   | Everything   | Doc + video | TikTok-first | **Captions only**       |
| Confidence highlighting | No           | No          | No           | **Yes**                 |
| Context priming         | No           | No          | No           | **Yes**                 |
| Works offline           | Partial      | No          | No           | **Yes (local Whisper)** |
| Video never uploaded    | —            | Uploads     | Uploads      | **Local by default**    |

## Stack

- **Tauri 2** (Rust) + React 19 + TypeScript + Tailwind v4 — same as SundayStage
- **whisper-rs** for local speech recognition (Phase 2)
- **ffmpeg** sidecar for video I/O + burn-in (Phases 1, 6.2)
- **ts-rs** auto-generates TypeScript bindings from Rust models

## What works today

- `npm run tauri dev` boots the app — Editor + Export tabs
- **Caption editor** with live confidence highlighting (4 tiers, accessibility underlines, focus mode, threshold slider, review progress) — against bundled sample data
- **Caption operations** (Rust, pure functions, 49 tests): split, merge, shift-all, edit-word, lock-word, accept-alternate, retime-word — all enforce timing invariants
- **Export** (Rust): SRT, VTT (with `<v>` speaker tags), ASS (full styling + libass-ready), TXT — all unit-tested against format quirks
- 12 auto-generated TypeScript bindings, wire-format-correct (`number` not `bigint` for i64 ms)

## What's next

See `docs/ARCHITECTURE.md` phase-status table. Highlights:

- **Phase 1** — Video import + audio extraction + waveform + timeline
- **Phase 2** — whisper-rs integration + per-word confidence extraction + cloud fallback
- **Phase 2.3** — confidence calibration suite (the empirical backing for the killer feature)
- **Phase 5** — styling editor + presets
- **Phase 6.2** — ffmpeg burn-in via libass

## Development

```bash
npm install
npm run tauri dev          # builds Rust + opens app

cd src-tauri
cargo test --lib           # 49 tests
cargo test --lib export_bindings   # regenerate TS bindings
```

## License

TBD — likely a source-available commercial license, given the standalone commercial intent.
