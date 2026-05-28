# Verbatim — Architecture

Last updated: 2026-05-28

## High-level flow

```mermaid
flowchart LR
  User[User] -- drag video --> Win[Verbatim window]
  Win --> Probe[ffmpeg probe metadata]
  Probe --> Audio[ffmpeg extract<br/>16kHz mono WAV]
  Audio --> Waveform[Waveform render]
  Audio --> ASR[Whisper-rs<br/>local transcribe]
  Glossary[Context + glossary] -.initial_prompt.-> ASR
  ASR --> Captions[Captions + per-word<br/>confidence]
  Captions --> Editor[Editor UI<br/>confidence highlighting]
  Editor --> Polish{Optional<br/>polish}
  Polish -- LLM --> Polished[Punctuation +<br/>capitalisation fixes]
  Polish -- skip --> Captions
  Polished --> Captions
  Captions --> Export{Export}
  Export --> SRT[SRT / VTT / ASS]
  Export --> Burnin[ffmpeg burn-in<br/>libass]
  Burnin --> MP4[Captioned MP4]

  classDef killer fill:#7dd3c4,stroke:#0f766e,color:#0f172a
  class ASR,Glossary,Editor killer
```

Killer-feature cells highlighted: ASR (with context priming) and Editor (with confidence highlighting).

## Data model

```mermaid
erDiagram
  Project ||--|{ Caption        : "ordered list"
  Project ||--o{ Speaker        : "diarization"
  Project ||--o{ GlossaryTerm   : "context"
  Project ||--o| Style          : "default style"
  Project ||--o{ HistoryEntry   : "undo stack"

  Caption ||--|{ Word           : "ordered words"
  Caption }o--o| Speaker        : "attributed"
  Caption }o--o| Style          : "override"
  Caption }o--o| GlossaryAutoCorrection : "applied"

  Word }o--o{ AlternateRead     : "ASR alternates"
```

### Project

| Field | Type | Notes |
|---|---|---|
| `id` | UUIDv7 | |
| `name` | string | derived from video filename initially |
| `video_path` | string | absolute path |
| `video_content_hash` | string | sha-256, for relink on path break |
| `video_duration_ms` | i64 | |
| `video_width`, `video_height` | i32 | |
| `video_fps` | f32 | |
| `audio_wav_path` | string | cached extracted audio |
| `language` | string | ISO 639-1; `auto` for autodetect |
| `default_style_id` | UUIDv7? | |
| `context_description` | string? | freeform — used as Whisper initial_prompt seed |
| `created_at`, `updated_at` | i64 | unix ms |

### Caption (one displayed subtitle line)

| Field | Type | Notes |
|---|---|---|
| `id` | UUIDv7 | |
| `project_id` | FK | |
| `start_ms`, `end_ms` | i64 | invariant: `start < end` |
| `text` | string | derived: `words.map(w=>w.text).join(" ")` |
| `speaker_id` | UUIDv7? | when diarization is on |
| `style_id` | UUIDv7? | per-caption override |
| `notes` | string? | editor note |
| `ai_generated` | bool | from ASR vs hand-typed |
| `last_edited_at` | i64 | |
| **Invariants** | | Captions never overlap; sorted by `start_ms` |

### Word

| Field | Type | Notes |
|---|---|---|
| `text` | string | |
| `start_ms`, `end_ms` | i64 | derived from Whisper |
| `confidence` | f32 | 0..100 normalized |
| `edited` | bool | user has changed this from ASR |
| `locked` | bool | user has confirmed (don't flag as uncertain anymore) |
| `alternates` | `AlternateRead[]` | top-3 Whisper alternates with their probs |

### GlossaryTerm

| Field | Type | Notes |
|---|---|---|
| `id` | UUIDv7 | |
| `project_id` | FK | |
| `term` | string | canonical form |
| `aliases` | `string[]` | misrecognitions auto-corrected to `term` |
| `definition` | string? | hover-display |
| `pronunciation_hint` | string? | for Whisper context |

### Style

| Field | Type | Notes |
|---|---|---|
| `id` | UUIDv7 | |
| `font_family`, `font_size`, `font_weight`, `italic` | | |
| `color_fg`, `outline_color`, `outline_width` | | |
| `shadow_color`, `shadow_offset_x`, `shadow_offset_y`, `shadow_blur` | | |
| `background_color`, `background_padding`, `background_radius` | | |
| `align_h`, `align_v` | | left/center/right × top/middle/bottom |
| `anchor` | string | 9-grid position |
| `max_width_pct` | f32 | |
| `line_spacing`, `letter_spacing` | | |
| `animation` | `AnimationSpec?` | fade, slide, karaoke, popup, none |

## Confidence tiers — the killer feature

Per-word confidence comes from the ASR model (log-probability of the chosen token, normalized to 0–100). The renderer assigns each word to one of four tiers:

| Tier | Range | Visual | Meaning |
|------|-------|--------|---------|
| 1 (high) | 85–100 | No highlight | The 92% you don't touch |
| 2 (medium) | 70–84 | Subtle amber background | Skimmable |
| 3 (low) | 50–69 | Clear amber + dotted underline | Demands a glance |
| 4 (very low) | 0–49 | Red-orange + wavy underline | Demands attention |

**Underlines are an accessibility fallback** — color alone isn't enough. Colorblind users still see SOMETHING.

Tier boundaries are NOT defaults pulled from thin air — they're calibrated against real transcripts. See `docs/CALIBRATION.md` (to be filled as we ship data).

## Operations (pure functions over Project state)

| Function | Signature | Notes |
|----------|-----------|-------|
| `splitCaption` | `(project, caption_id, at_word_index)` | one caption → two |
| `mergeCaptions` | `(project, [caption_ids])` | adjacent only |
| `shiftAllCaptions` | `(project, offset_ms)` | bulk nudge |
| `editWord` | `(project, caption_id, word_index, new_text)` | marks `edited` |
| `retimeWord` | `(project, caption_id, word_index, start, end)` | manual timing |
| `lockWord` | `(project, caption_id, word_index)` | removes confidence highlight |
| `acceptAlternate` | `(project, caption_id, word_index, alternate_index)` | from tooltip |
| `regenerateCaption` | `(project, caption_id)` | re-run ASR on this caption's time range |

All operations validate invariants and return a new `Project` state. Undo is trivial: keep the previous state. History is capped (default 100).

## Project file format

`.verbatim` files are SQLite databases — one file per project. Same engine as the in-memory data model; just persisted. This makes loading instant and avoids JSON-parse cost for projects with 5000+ captions.

Caveat for path-stability: if the user moves their video file, Verbatim detects the missing path on open, hashes candidate files in common locations, and offers to relink. Same pattern as SundayStage's MediaAsset relink (Phase 7.2 there).

## Phase status (May 2026)

- [x] Phase 0 — Scaffold + design tokens + confidence color scale
- [x] Phase 1.1 — Video import: ffprobe metadata, format validation, content-hash relink, `.verbatim` SQLite file format
- [x] Phase 1.2 — Audio extraction command + multi-zoom waveform peaks + Canvas waveform component
- [ ] Phase 1.3 — Full timeline (caption track + ruler + J/K/L) — partial (waveform + click-seek done)
- [x] Phase 2.1 — ASR abstraction (`AsrProvider`), Whisper model registry, feature-gated `LocalWhisperProvider` (`--features whisper`), segment→caption captionizer
- [x] Phase 2.2 — Cloud provider response normalization (OpenAI / AssemblyAI / Deepgram), confidence parity across backends
- [x] Phase 2.3 — Per-word confidence normalization (logprob → 0..100, single calibrated curve) + `docs/CALIBRATION.md`
- [x] Phase 3.1 — Caption data model + operations *(in scaffold)*
- [ ] Phase 3.2 — Editor UX
- [~] Phase 3.3 — Confidence highlighting (killer feature #1) — editor demo live; calibration pending real data
- [ ] Phase 3.4 — Context priming + glossary (killer feature #2)
- [ ] Phase 4 — Polish + diarization + smart suggestions
- [ ] Phase 5 — Styling system + presets
- [x] Phase 6.1 — SRT / VTT / ASS export *(in scaffold)*
- [ ] Phase 6.2 — Burn-in render via libass
- [ ] Phase 6.3 — Platform export presets
- [ ] Phase 7 — Translation, filler removal, find & replace
- [ ] Phase 8 — Sunday Account integration (optional)
- [ ] Phase 9 — Onboarding + distribution + landing site
