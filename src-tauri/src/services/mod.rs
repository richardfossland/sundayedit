//! Business logic services. Each module is pure (functional) where
//! possible — input state, output new state, no shared mutation.
//!
//!   - `operations`   — caption split/merge/edit/shift (Phase 3.1)
//!   - `export`       — SRT/VTT/ASS writers (Phase 6.1)
//!   - `video`        — ffprobe metadata, format validation, content hash (Phase 1.1)
//!   - `project_file` — .verbatim SQLite save/load (Phase 1.1)
//!   - `waveform`     — audio extraction + multi-zoom peaks (Phase 1.2)
//!   - `asr`          — speech recognition: confidence, models, cloud, local (Phase 2)
//!   - `glossary`     — post-transcription auto-correction (Phase 3.4)
//!   - `style_presets`— bundled subtitle style presets (Phase 5.1/5.3)

pub mod operations;
pub mod export;
pub mod video;
pub mod project_file;
pub mod waveform;
pub mod asr;
pub mod glossary;
pub mod style_presets;
