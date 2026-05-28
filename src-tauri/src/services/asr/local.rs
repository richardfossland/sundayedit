//! Local Whisper provider — Phase 2.1.
//!
//! Real transcription runs on-device via `whisper-rs` (whisper.cpp
//! bindings). Building whisper.cpp needs a C/C++ toolchain + cmake and
//! pulls in a multi-GB model at runtime, so it sits behind the optional
//! `whisper` cargo feature:
//!
//!   cargo build --features whisper      # real local transcription
//!   cargo build                         # everything else; ASR stubbed
//!
//! The default build (and the test suite) compile WITHOUT building
//! whisper.cpp. The stub returns a clear, actionable error so the UI can
//! tell the user "this build doesn't include local transcription —
//! rebuild with --features whisper, or use a cloud provider."
//!
//! GPU acceleration (Metal on macOS, CUDA on Windows) is selected by
//! whisper-rs's build features when the `whisper` feature is on.

use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::services::asr::model::WhisperModel;
use crate::services::asr::{AsrOptions, AsrProvider, Transcript, TranscribeProgress};

pub struct LocalWhisperProvider {
    pub model: WhisperModel,
    pub model_path: PathBuf,
}

impl LocalWhisperProvider {
    pub fn new(model: WhisperModel, models_dir: &Path) -> Self {
        Self {
            model,
            model_path: model.path_in(models_dir),
        }
    }
}

// ── Real implementation (feature = "whisper") ────────────────────────────────
#[cfg(feature = "whisper")]
impl AsrProvider for LocalWhisperProvider {
    fn name(&self) -> String {
        format!("Local · {}", self.model.filename())
    }

    fn transcribe(
        &self,
        audio_path: &Path,
        opts: &AsrOptions,
        progress: &mut dyn FnMut(TranscribeProgress),
    ) -> AppResult<Transcript> {
        use crate::error::AppError;
        use crate::services::asr::confidence::word_confidence_from_token_logprobs;
        use crate::services::asr::{Segment, TranscribedWord};
        use crate::services::waveform::read_wav_samples;
        use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

        progress(TranscribeProgress::Preparing);

        if !self.model_path.is_file() {
            return Err(AppError::Internal(format!(
                "Whisper model not found at {}. Download it first.",
                self.model_path.display()
            )));
        }

        let ctx = WhisperContext::new_with_params(
            &self.model_path.to_string_lossy(),
            WhisperContextParameters::default(),
        )
        .map_err(|e| AppError::Internal(format!("failed to load Whisper model: {e}")))?;

        let (samples, _sr) = read_wav_samples(audio_path)?;

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: opts.beam_size,
            patience: 1.0,
        });
        params.set_token_timestamps(true);
        params.set_max_len(1); // word-level segments
        params.set_split_on_word(true);
        if opts.language != "auto" {
            params.set_language(Some(&opts.language));
        }
        if let Some(prompt) = opts.initial_prompt() {
            params.set_initial_prompt(&prompt);
        }

        let mut state = ctx
            .create_state()
            .map_err(|e| AppError::Internal(format!("whisper state: {e}")))?;
        state
            .full(params, &samples)
            .map_err(|e| AppError::Internal(format!("whisper transcribe: {e}")))?;

        let n_segments = state.full_n_segments()
            .map_err(|e| AppError::Internal(format!("segment count: {e}")))?;

        let mut words: Vec<TranscribedWord> = Vec::new();
        for i in 0..n_segments {
            let text = state.full_get_segment_text(i)
                .map_err(|e| AppError::Internal(format!("segment text: {e}")))?;
            let t0 = state.full_get_segment_t0(i).unwrap_or(0); // centiseconds
            let t1 = state.full_get_segment_t1(i).unwrap_or(t0);

            // Average token logprobs across the segment's tokens.
            let n_tokens = state.full_n_tokens(i).unwrap_or(0);
            let mut logprobs = Vec::new();
            for j in 0..n_tokens {
                if let Ok(prob) = state.full_get_token_prob(i, j) {
                    // whisper-rs exposes token probability (0..1); convert to logprob
                    logprobs.push(prob.max(1e-6).ln());
                }
            }
            let confidence = word_confidence_from_token_logprobs(&logprobs);

            let text = text.trim().to_string();
            if text.is_empty() { continue; }
            words.push(TranscribedWord {
                text,
                start_ms: t0 * 10, // cs → ms
                end_ms: t1 * 10,
                confidence,
            });

            let frac = (i + 1) as f32 / n_segments as f32;
            progress(TranscribeProgress::Segment {
                fraction: frac,
                segment: Segment {
                    start_ms: t0 * 10,
                    end_ms: t1 * 10,
                    words: words.last().cloned().into_iter().collect(),
                },
            });
        }

        progress(TranscribeProgress::Done);

        let language = if opts.language == "auto" { "auto".to_string() } else { opts.language.clone() };
        Ok(Transcript {
            language,
            backend: self.name(),
            segments: vec![Segment {
                start_ms: words.first().map(|w| w.start_ms).unwrap_or(0),
                end_ms: words.last().map(|w| w.end_ms).unwrap_or(0),
                words,
            }],
        })
    }
}

// ── Stub implementation (default build, no whisper.cpp) ───────────────────────
#[cfg(not(feature = "whisper"))]
impl AsrProvider for LocalWhisperProvider {
    fn name(&self) -> String {
        format!("Local · {} (unavailable)", self.model.filename())
    }

    fn transcribe(
        &self,
        _audio_path: &Path,
        _opts: &AsrOptions,
        _progress: &mut dyn FnMut(TranscribeProgress),
    ) -> AppResult<Transcript> {
        Err(crate::error::AppError::Internal(
            "This build of Verbatim does not include local transcription. \
             Rebuild with `--features whisper`, or configure a cloud provider in Settings."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_name_includes_model() {
        let dir = std::path::Path::new("/tmp/models");
        let p = LocalWhisperProvider::new(WhisperModel::LargeV3Turbo, dir);
        assert!(p.name().contains("large-v3-turbo"));
    }

    #[test]
    fn model_path_resolves_under_dir() {
        let dir = std::path::Path::new("/data/models");
        let p = LocalWhisperProvider::new(WhisperModel::Base, dir);
        assert_eq!(p.model_path, dir.join("ggml-base.bin"));
    }

    // Without the `whisper` feature, transcribe returns a clear error
    // rather than panicking — the UI relies on this to guide the user.
    #[cfg(not(feature = "whisper"))]
    #[test]
    fn stub_returns_actionable_error() {
        let dir = std::path::Path::new("/tmp/models");
        let p = LocalWhisperProvider::new(WhisperModel::Base, dir);
        let mut noop = |_p: TranscribeProgress| {};
        let err = p.transcribe(Path::new("/tmp/x.wav"), &AsrOptions::default(), &mut noop).unwrap_err();
        assert_eq!(err.code(), "internal");
        assert!(err.to_string().contains("--features whisper"));
    }
}
