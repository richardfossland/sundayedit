//! Verbatim — AI-assisted video captioning for desktop.
//!
//! Entry point. Wires up logging, registers all Tauri command handlers,
//! and runs the Tauri runtime. The actual work happens in `services::*`.

pub mod commands;
pub mod error;
pub mod model;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Caption operations (Phase 3.1)
            commands::operations::op_split_caption,
            commands::operations::op_merge_captions,
            commands::operations::op_shift_all_captions,
            commands::operations::op_edit_word,
            commands::operations::op_lock_word,
            commands::operations::op_accept_alternate,
            commands::operations::op_retime_word,
            commands::operations::op_apply_glossary,
            // Export (Phase 6.1)
            commands::export::export_srt,
            commands::export::export_vtt,
            commands::export::export_ass,
            commands::export::export_txt,
            // Project lifecycle + video import (Phase 1)
            commands::project::video_probe,
            commands::project::project_create_from_video,
            commands::project::project_save,
            commands::project::project_open,
            commands::project::waveform_compute,
            commands::project::project_relink,
            commands::project::accepted_media_extensions,
            // ASR / transcription (Phase 2)
            commands::asr::asr_list_models,
            commands::asr::asr_downloaded_models,
            commands::asr::asr_transcribe_local,
            // Styling (Phase 5)
            commands::style::style_list_presets,
            // Burn-in + platform export (Phase 6.2 / 6.3)
            commands::render::export_list_presets,
            commands::render::export_validate,
            commands::render::burnin_render,
            commands::render::burnin_render_preset,
            // Find/replace + filler cleanup (Phase 7)
            commands::cleanup::find_in_project,
            commands::cleanup::replace_in_project,
            commands::cleanup::bulk_delete_captions,
            commands::cleanup::bulk_set_speaker,
            commands::cleanup::detect_fillers,
            commands::cleanup::detect_silences,
            commands::cleanup::apply_ripple_cuts,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
