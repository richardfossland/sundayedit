//! ASR Tauri commands — Phase 2.
//!
//! Lists models, reports which are downloaded, and runs transcription.
//! Transcription converts the resulting Transcript → Caption[] via the
//! pure captionizer, so the renderer gets editor-ready captions with
//! confidence already populated.

use std::path::Path;

use tauri::Emitter;

use crate::error::AppResult;
use crate::model::Caption;
use crate::services::asr::captionize::{captionize, CaptionizeOptions};
use crate::services::asr::local::LocalWhisperProvider;
use crate::services::asr::model::{catalog, WhisperModel, WhisperModelInfo};
use crate::services::asr::{AsrOptions, AsrProvider, TranscribeProgress};

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// The model catalog for the picker.
#[tauri::command]
pub fn asr_list_models() -> Vec<WhisperModelInfo> {
    catalog()
}

/// Which models are already downloaded into `models_dir`.
#[tauri::command]
pub fn asr_downloaded_models(models_dir: String) -> Vec<WhisperModel> {
    let dir = Path::new(&models_dir);
    WhisperModel::all()
        .into_iter()
        .filter(|m| m.is_downloaded(dir))
        .collect()
}

/// Transcribe an audio WAV with the local model and return editor-ready
/// captions. Streams `transcribe-progress` events to the window.
///
/// Without the `whisper` feature the provider returns a clear error that
/// the renderer surfaces ("rebuild with --features whisper or use cloud").
#[tauri::command]
pub fn asr_transcribe_local(
    window: tauri::Window,
    audio_path: String,
    models_dir: String,
    model: WhisperModel,
    options: AsrOptions,
) -> AppResult<Vec<Caption>> {
    let provider = LocalWhisperProvider::new(model, Path::new(&models_dir));

    let mut on_progress = |p: TranscribeProgress| {
        let _ = window.emit("transcribe-progress", &p);
    };

    let transcript = provider.transcribe(Path::new(&audio_path), &options, &mut on_progress)?;

    let mut counter = 0usize;
    let captions = captionize(
        &transcript,
        CaptionizeOptions::default(),
        now_ms(),
        |_idx| {
            counter += 1;
            new_id()
        },
    );
    Ok(captions)
}
