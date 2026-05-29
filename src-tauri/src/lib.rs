//! SundayEdit — AI-assisted video captioning for desktop.
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
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::asr::DownloadControl::default());

    // Auto-update + relaunch are desktop-only (Phase 9.2).
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            // Caption operations (Phase 3.1)
            commands::operations::op_split_caption,
            commands::operations::op_merge_captions,
            commands::operations::op_shift_all_captions,
            commands::operations::op_move_caption,
            commands::operations::op_resize_caption,
            commands::operations::op_edit_word,
            commands::operations::op_lock_word,
            commands::operations::op_accept_alternate,
            commands::operations::op_retime_word,
            commands::operations::op_apply_glossary,
            // AI glossary suggestions (Phase 3.4 mode 3)
            commands::glossary::glossary_suggest_estimate,
            commands::glossary::glossary_suggest,
            // Export (Phase 6.1)
            commands::export::export_srt,
            commands::export::export_vtt,
            commands::export::export_ass,
            commands::export::export_txt,
            commands::export::export_json,
            commands::export::save_export,
            // Project lifecycle + video import (Phase 1)
            commands::project::video_probe,
            commands::project::project_create_from_video,
            commands::project::project_save,
            commands::project::project_open,
            commands::project::waveform_compute,
            commands::project::extract_audio,
            commands::project::project_relink,
            commands::project::accepted_media_extensions,
            // ASR / transcription (Phase 2)
            commands::asr::asr_list_models,
            commands::asr::cloud_providers,
            commands::asr::cloud_cost_estimate,
            commands::asr::cloud_transcribe,
            commands::asr::asr_downloaded_models,
            commands::asr::asr_download_model,
            commands::asr::asr_cancel_download,
            commands::asr::asr_transcribe_local,
            // API key storage (Phase 2.2)
            commands::secrets::secret_set,
            commands::secrets::secret_delete,
            commands::secrets::secret_status,
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
            // AI punctuation polish (Phase 4.1)
            commands::polish::polish_estimate,
            commands::polish::polish_captions,
            // AI smart suggestions (Phase 4.3)
            commands::suggest::suggest_estimate,
            commands::suggest::suggest_captions,
            commands::suggest::apply_suggestion,
            // AI translation (Phase 7.1)
            commands::translate::translate_supported_languages,
            commands::translate::translate_estimate,
            commands::translate::translate_captions,
            // Speaker diarization (Phase 4.2)
            commands::diarize::diarize_run,
            commands::diarize::speaker_merge,
            commands::diarize::speaker_rename,
            commands::diarize::speaker_set_color,
            // AI social clips (SundayEdit)
            commands::clips::clips_estimate,
            commands::clips::clips_generate,
            commands::clips::clips_apply_plan,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
