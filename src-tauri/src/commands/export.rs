//! Export Tauri commands. The renderer passes the current project
//! state in — Rust generates the formatted string.

use crate::error::{AppError, AppResult};
use crate::model::Project;
use crate::services::export::{
    build_docx, write_ass, write_json, write_srt, write_txt, write_vtt, JsonOptions, SrtOptions,
    TxtOptions, VttOptions,
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
pub fn export_json(project: Project, strip_empty: bool) -> AppResult<String> {
    Ok(write_json(&project, JsonOptions { strip_empty }))
}

/// Regenerate `format` server-side and write it to `path` (chosen by the OS
/// save dialog). One command for every format so the renderer never has to
/// handle file bytes — and DOCX (binary) is covered too.
#[tauri::command]
pub fn save_export(
    project: Project,
    path: String,
    format: String,
    include_speakers: bool,
    strip_empty: bool,
) -> AppResult<()> {
    let bytes: Vec<u8> = match format.as_str() {
        "srt" => write_srt(
            &project,
            SrtOptions {
                include_speakers,
                strip_empty,
            },
        )
        .into_bytes(),
        "vtt" => write_vtt(
            &project,
            VttOptions {
                include_speakers,
                strip_empty,
            },
        )
        .into_bytes(),
        "ass" => write_ass(&project).into_bytes(),
        "txt" => write_txt(
            &project,
            TxtOptions {
                include_speakers,
                strip_empty,
            },
        )
        .into_bytes(),
        "json" => write_json(&project, JsonOptions { strip_empty }).into_bytes(),
        "docx" => build_docx(
            &project,
            TxtOptions {
                include_speakers,
                strip_empty,
            },
        )?,
        other => {
            return Err(AppError::Validation(format!(
                "unknown export format: {other}"
            )))
        }
    };
    std::fs::write(&path, bytes)?;
    Ok(())
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
