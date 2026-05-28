//! Export Tauri commands. The renderer passes the current project
//! state in — Rust generates the formatted string.

use crate::error::AppResult;
use crate::model::Project;
use crate::services::export::{
    write_ass, write_srt, write_txt, write_vtt, SrtOptions, TxtOptions, VttOptions,
};

#[tauri::command]
pub fn export_srt(
    project: Project,
    include_speakers: bool,
    strip_empty: bool,
) -> AppResult<String> {
    Ok(write_srt(
        &project,
        SrtOptions {
            include_speakers,
            strip_empty,
        },
    ))
}

#[tauri::command]
pub fn export_vtt(
    project: Project,
    include_speakers: bool,
    strip_empty: bool,
) -> AppResult<String> {
    Ok(write_vtt(
        &project,
        VttOptions {
            include_speakers,
            strip_empty,
        },
    ))
}

#[tauri::command]
pub fn export_ass(project: Project) -> AppResult<String> {
    Ok(write_ass(&project))
}

#[tauri::command]
pub fn export_txt(
    project: Project,
    include_speakers: bool,
    strip_empty: bool,
) -> AppResult<String> {
    Ok(write_txt(
        &project,
        TxtOptions {
            include_speakers,
            strip_empty,
        },
    ))
}
