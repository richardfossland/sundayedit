//! Find/replace + filler/silence cleanup Tauri commands — Phase 7.

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppResult;
use crate::model::Project;
use crate::services::filler::{self, FillerHit, SilenceGap};
use crate::services::find_replace::{self, FindMatch, FindOptions};

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Find & replace (7.3) ───────────────────────────────────────────────────────

#[tauri::command]
pub fn find_in_project(project: Project, options: FindOptions) -> AppResult<Vec<FindMatch>> {
    find_replace::find_all(&project, &options)
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ReplaceResult.ts")]
pub struct ReplaceResult {
    pub project: Project,
    pub count: usize,
}

#[tauri::command]
pub fn replace_in_project(
    project: Project,
    options: FindOptions,
    replacement: String,
) -> AppResult<ReplaceResult> {
    let (project, count) = find_replace::replace_all(&project, &options, &replacement, now_ms())?;
    Ok(ReplaceResult { project, count })
}

#[tauri::command]
pub fn bulk_delete_captions(project: Project, caption_ids: Vec<String>) -> AppResult<Project> {
    Ok(find_replace::bulk_delete(&project, &caption_ids, now_ms()))
}

#[tauri::command]
pub fn bulk_set_speaker(
    project: Project,
    caption_ids: Vec<String>,
    speaker_id: Option<String>,
) -> AppResult<Project> {
    Ok(find_replace::bulk_set_speaker(
        &project,
        &caption_ids,
        speaker_id,
        now_ms(),
    ))
}

// ── Filler / silence removal (7.2) ──────────────────────────────────────────────

#[tauri::command]
pub fn detect_fillers(project: Project, language: String) -> AppResult<Vec<FillerHit>> {
    Ok(filler::detect_fillers(&project, &language))
}

#[tauri::command]
pub fn detect_silences(project: Project, min_gap_ms: i64) -> AppResult<Vec<SilenceGap>> {
    Ok(filler::detect_silences(&project, min_gap_ms))
}

/// Apply ripple cuts from approved (start_ms, end_ms) ranges.
#[tauri::command]
pub fn apply_ripple_cuts(project: Project, cuts: Vec<(i64, i64)>) -> AppResult<Project> {
    filler::apply_ripple_cuts(&project, &cuts, now_ms())
}
