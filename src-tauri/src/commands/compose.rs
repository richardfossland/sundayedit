//! Multi-track COMPOSE render Tauri commands.
//!
//!   - `compose_render` — flatten the timeline to `output` via the
//!     `filter_complex` pipeline (or the single-track burn-in shortcut when the
//!     timeline is simple). Streams `compose-render-progress` events and honours
//!     a shared cancel flag, mirroring `reel_render_all`.
//!   - `compose_cancel` — flip the cancel flag.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::AppResult;
use crate::model::Project;
use crate::services::compose::{self, ComposeSettings};

/// Shared cancel flag for the in-flight compose render (one at a time).
/// Managed by the Tauri runtime, mirroring `ReelRenderControl`.
#[derive(Default)]
pub struct ComposeRenderControl {
    cancel: Arc<AtomicBool>,
}

/// Render the whole timeline to `output` with the given `settings`. Long-
/// running; runs the ffmpeg pipeline on a blocking thread so it never starves
/// the Tokio executor. Progress is emitted from inside via `window.emit`;
/// cancel is polled while parsing ffmpeg's `-progress` stream.
#[tauri::command]
pub async fn compose_render(
    window: tauri::Window,
    control: tauri::State<'_, ComposeRenderControl>,
    project: Project,
    output: String,
    settings: ComposeSettings,
) -> AppResult<()> {
    let cancel = control.cancel.clone();
    cancel.store(false, Ordering::Relaxed);

    tokio::task::spawn_blocking(move || {
        compose::run_compose(&window, &project, Path::new(&output), &settings, cancel)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("compose render task join: {e}")))?
}

/// Render a FAST LOW-RES preview proxy of the whole timeline to `output`.
/// Settings are derived from the project (`compose::proxy_settings`); progress
/// is emitted on `compose-proxy-progress`. Reuses the same `ComposeRenderControl`
/// cancel flag (only one compose/proxy render runs at a time).
#[tauri::command]
pub async fn compose_preview_proxy(
    window: tauri::Window,
    control: tauri::State<'_, ComposeRenderControl>,
    project: Project,
    output: String,
) -> AppResult<()> {
    let cancel = control.cancel.clone();
    cancel.store(false, Ordering::Relaxed);

    tokio::task::spawn_blocking(move || {
        compose::run_compose_proxy(&window, &project, Path::new(&output), cancel)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(format!("proxy render task join: {e}")))?
}

/// Cancel the in-flight compose render, if any. The ffmpeg child is killed at
/// the next progress line and the partial output is reported as an error.
#[tauri::command]
pub fn compose_cancel(control: tauri::State<'_, ComposeRenderControl>) {
    control.cancel.store(true, Ordering::Relaxed);
}
