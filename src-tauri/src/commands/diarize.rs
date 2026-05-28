//! Speaker diarization Tauri commands — Phase 4.2.
//!
//!   - `diarize_run` — async. Runs the (feature-gated) engine on the audio
//!     and assigns speakers to captions, returning the updated project.
//!   - `speaker_merge` / `speaker_rename` / `speaker_set_color` — pure
//!     roster edits the Speakers panel drives.

use std::path::PathBuf;

use crate::error::AppResult;
use crate::model::Project;
use crate::services::diarize;

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Detect speakers from the project's extracted audio and attribute every
/// caption. Best-effort — the UI must prompt the user to verify.
#[tauri::command]
pub async fn diarize_run(project: Project, audio_path: String) -> AppResult<Project> {
    let turns = diarize::run_diarization(&PathBuf::from(audio_path))?;
    Ok(diarize::assign_speakers(&project, &turns, now_ms()))
}

#[tauri::command]
pub fn speaker_merge(project: Project, keep_id: String, remove_id: String) -> AppResult<Project> {
    diarize::merge_speakers(&project, &keep_id, &remove_id, now_ms())
}

#[tauri::command]
pub fn speaker_rename(project: Project, speaker_id: String, name: String) -> AppResult<Project> {
    diarize::rename_speaker(&project, &speaker_id, &name, now_ms())
}

#[tauri::command]
pub fn speaker_set_color(
    project: Project,
    speaker_id: String,
    color_hex: String,
) -> AppResult<Project> {
    diarize::set_speaker_color(&project, &speaker_id, &color_hex, now_ms())
}
