//! Automatic speech recognition (ASR) layer — Phase 2.
//!
//! One trait (`AsrProvider`) abstracts over local Whisper and the cloud
//! providers. Everything downstream — the captionizer, the editor, the
//! confidence highlights — works against the normalized `Transcript`
//! type and never cares which backend produced it.
//!
//! Submodules:
//!   - `confidence`  — logprob → 0..100 normalization (killer feature #1)
//!   - `model`       — Whisper model registry (sizes, URLs, recommend)
//!   - `captionize`  — Transcript → Caption[] with slide-breaking
//!   - `cloud`       — cloud provider response normalization
//!   - `local`       — LocalWhisperProvider (feature-gated on `whisper`)

pub mod calibration;
pub mod captionize;
pub mod cloud;
pub mod confidence;
pub mod download;
pub mod local;
pub mod model;

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppResult;

/// A fully normalized transcription result. The unit the rest of the app
/// consumes, regardless of backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Transcript.ts")]
pub struct Transcript {
    pub language: String,
    pub segments: Vec<Segment>,
    /// Which backend produced this — shown in the UI ("Local · large-v3-turbo").
    pub backend: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Segment.ts")]
pub struct Segment {
    #[ts(type = "number")]
    pub start_ms: i64,
    #[ts(type = "number")]
    pub end_ms: i64,
    pub words: Vec<TranscribedWord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/TranscribedWord.ts")]
pub struct TranscribedWord {
    pub text: String,
    #[ts(type = "number")]
    pub start_ms: i64,
    #[ts(type = "number")]
    pub end_ms: i64,
    /// Already normalized to 0..100 via `confidence::*`.
    pub confidence: f32,
}

/// Options that steer a transcription run. Built from the project's
/// language + context/glossary (the glossary→initial_prompt assembly is
/// Phase 3.4; the field is here so the plumbing exists).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/AsrOptions.ts")]
pub struct AsrOptions {
    /// ISO 639-1 or "auto" for language autodetect.
    pub language: String,
    /// Whisper beam size — higher = more accurate, slower. Default 5.
    pub beam_size: i32,
    /// Bias recognition toward these terms (Whisper initial_prompt /
    /// AssemblyAI word_boost / Deepgram keywords). Phase 3.4 populates it.
    pub priming_terms: Vec<String>,
    /// Freeform context sentence prepended to the initial_prompt.
    pub context_description: Option<String>,
}

impl Default for AsrOptions {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            beam_size: 5,
            priming_terms: Vec::new(),
            context_description: None,
        }
    }
}

impl AsrOptions {
    /// Assemble the Whisper `initial_prompt` (≤ 224 tokens — we keep it
    /// short). Cloud providers use `priming_terms` directly instead.
    ///
    /// Pure + testable: "context. Names and terms: a, b, c."
    pub fn initial_prompt(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ctx) = &self.context_description {
            let ctx = ctx.trim();
            if !ctx.is_empty() {
                parts.push(ctx.to_string());
            }
        }
        if !self.priming_terms.is_empty() {
            // Cap the term list so we stay well under Whisper's prompt window.
            let terms: Vec<&str> = self
                .priming_terms
                .iter()
                .map(|s| s.as_str())
                .take(48)
                .collect();
            parts.push(format!("Names and terms to expect: {}.", terms.join(", ")));
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }
}

/// Progress events streamed to the UI during a run.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../src/lib/bindings/TranscribeProgress.ts")]
pub enum TranscribeProgress {
    /// Model is loading / warming up.
    Preparing,
    /// Inference is running — fraction is 0..1 of the audio decoded so far
    /// (whisper's own progress callback, fired between encoder steps). This is
    /// the long phase; without it the UI sat on "preparing" for the whole run.
    Running { fraction: f32 },
    /// A segment finished — fraction is 0..1 of total audio processed.
    Segment { fraction: f32, segment: Segment },
    /// Done.
    Done,
}

/// The abstraction both local and cloud backends implement.
pub trait AsrProvider {
    /// Human-readable backend name for the UI ("Local · large-v3-turbo").
    fn name(&self) -> String;

    /// Transcribe `audio_path` (a 16 kHz mono WAV from Phase 1.2).
    /// `progress` is shared + 'static so the local provider can hand a clone to
    /// whisper.cpp's progress callback (which outlives any plain borrow);
    /// `cancel` is polled by the abort callback between encoder steps — raise
    /// it to stop a run (surfaces as a "cancelled" error).
    fn transcribe(
        &self,
        audio_path: &Path,
        opts: &AsrOptions,
        progress: std::sync::Arc<dyn Fn(TranscribeProgress) + Send + Sync>,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> AppResult<Transcript>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_prompt_empty_when_no_context() {
        let o = AsrOptions::default();
        assert_eq!(o.initial_prompt(), None);
    }

    #[test]
    fn initial_prompt_with_context_only() {
        let o = AsrOptions {
            context_description: Some("A sermon about grace.".into()),
            ..Default::default()
        };
        assert_eq!(o.initial_prompt().as_deref(), Some("A sermon about grace."));
    }

    #[test]
    fn initial_prompt_with_terms_only() {
        let o = AsrOptions {
            priming_terms: vec!["kerygma".into(), "soteriologi".into()],
            ..Default::default()
        };
        assert_eq!(
            o.initial_prompt().as_deref(),
            Some("Names and terms to expect: kerygma, soteriologi."),
        );
    }

    #[test]
    fn initial_prompt_combines_context_and_terms() {
        let o = AsrOptions {
            context_description: Some("A theology lecture.".into()),
            priming_terms: vec!["kerygma".into()],
            ..Default::default()
        };
        assert_eq!(
            o.initial_prompt().as_deref(),
            Some("A theology lecture. Names and terms to expect: kerygma."),
        );
    }

    #[test]
    fn initial_prompt_caps_term_count() {
        let terms: Vec<String> = (0..100).map(|i| format!("term{i}")).collect();
        let o = AsrOptions {
            priming_terms: terms,
            ..Default::default()
        };
        let prompt = o.initial_prompt().unwrap();
        // 48-term cap → term0..term47 present, term48 absent
        assert!(prompt.contains("term47"));
        assert!(!prompt.contains("term48"));
    }
}
