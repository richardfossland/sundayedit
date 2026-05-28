//! Burn-in + platform-export Tauri commands — Phase 6.2 / 6.3.

use std::path::Path;

use crate::error::AppResult;
use crate::model::Project;
use crate::services::burnin::{self, BurnInOptions};
use crate::services::export_presets::{self, ExportPreset, ExportWarning};

/// The platform export-preset catalog for the export UI.
#[tauri::command]
pub fn export_list_presets() -> Vec<ExportPreset> {
    export_presets::catalog()
}

/// Validate a project against a preset's platform rules (duration,
/// aspect, captions present) — shown before the user commits to a render.
#[tauri::command]
pub fn export_validate(project: Project, preset: ExportPreset) -> Vec<ExportWarning> {
    export_presets::validate(&project, &preset)
}

/// Burn captions into the video at `output`. Long-running; errors clearly
/// if ffmpeg is unavailable. (Progress streaming via events lands when we
/// parse ffmpeg's -progress output — Phase 6.2 polish.)
#[tauri::command]
pub fn burnin_render(
    project: Project,
    output: String,
    options: BurnInOptions,
) -> AppResult<()> {
    burnin::render(&project, Path::new(&output), &options)
}

/// Burn-in using a platform preset's dimensions/bitrate in one call.
#[tauri::command]
pub fn burnin_render_preset(
    project: Project,
    output: String,
    preset: ExportPreset,
) -> AppResult<()> {
    let options = preset.to_burnin_options();
    burnin::render(&project, Path::new(&output), &options)
}
