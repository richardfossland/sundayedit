//! Caption-editing Tauri commands. The pure operations live in
//! `services::operations`; these wrappers just shuttle data across IPC.
//!
//! Project state itself is kept in the renderer (TanStack Query cache)
//! for v1 — the Rust side is stateless for these operations. When we
//! add SQLite project persistence, the renderer will push state changes
//! through here for persistence too.

use serde::Serialize;
use ts_rs::TS;

use crate::error::AppResult;
use crate::model::Project;
use crate::services::{operations, glossary};
use crate::services::glossary::GlossaryCorrection;

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[tauri::command]
pub fn op_split_caption(
    project: Project,
    caption_id: String,
    at_word_index: usize,
) -> AppResult<Project> {
    operations::split_caption(&project, &caption_id, at_word_index, now_ms(), new_id())
}

#[tauri::command]
pub fn op_merge_captions(project: Project, caption_ids: Vec<String>) -> AppResult<Project> {
    let refs: Vec<&str> = caption_ids.iter().map(|s| s.as_str()).collect();
    operations::merge_captions(&project, &refs, now_ms())
}

#[tauri::command]
pub fn op_shift_all_captions(project: Project, offset_ms: i64) -> AppResult<Project> {
    operations::shift_all_captions(&project, offset_ms, now_ms())
}

#[tauri::command]
pub fn op_edit_word(
    project: Project,
    caption_id: String,
    word_index: usize,
    new_text: String,
) -> AppResult<Project> {
    operations::edit_word(&project, &caption_id, word_index, &new_text, now_ms())
}

#[tauri::command]
pub fn op_lock_word(
    project: Project,
    caption_id: String,
    word_index: usize,
    locked: bool,
) -> AppResult<Project> {
    operations::lock_word(&project, &caption_id, word_index, locked, now_ms())
}

#[tauri::command]
pub fn op_accept_alternate(
    project: Project,
    caption_id: String,
    word_index: usize,
    alternate_index: usize,
) -> AppResult<Project> {
    operations::accept_alternate(&project, &caption_id, word_index, alternate_index, now_ms())
}

#[tauri::command]
pub fn op_retime_word(
    project: Project,
    caption_id: String,
    word_index: usize,
    new_start_ms: i64,
    new_end_ms: i64,
) -> AppResult<Project> {
    operations::retime_word(&project, &caption_id, word_index, new_start_ms, new_end_ms, now_ms())
}

/// Result of a glossary auto-correction pass: the new project plus the
/// list of corrections so the UI can show "we fixed 4 terms — review?".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/GlossaryApplyResult.ts")]
pub struct GlossaryApplyResult {
    pub project: Project,
    pub corrections: Vec<GlossaryCorrection>,
}

#[tauri::command]
pub fn op_apply_glossary(project: Project) -> AppResult<GlossaryApplyResult> {
    let (project, corrections) = glossary::apply_glossary(&project, now_ms());
    Ok(GlossaryApplyResult { project, corrections })
}
