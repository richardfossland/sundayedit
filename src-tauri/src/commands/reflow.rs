//! Caption readability (re-flow) Tauri commands — Phase 7.
//!
//! The pure analysis/repair logic lives in `services::reflow`; these
//! wrappers just shuttle data across IPC. Until now the reflow service was
//! unreachable from the frontend — these commands surface it so the editor
//! can flag CPS/line-length/line-count/min-duration violations and one-click
//! auto-split the offenders into broadcast-compliant captions.

use crate::error::AppResult;
use crate::model::Project;
use crate::services::reflow::{self, ReflowConfig, ReflowIssue};

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Flag every caption that breaks a readability limit in `cfg`. Empty = clean.
#[tauri::command]
pub fn reflow_analyze(project: Project, cfg: ReflowConfig) -> AppResult<Vec<ReflowIssue>> {
    Ok(reflow::analyze(&project, &cfg))
}

/// Auto-repair flagged captions by splitting them at word boundaries into the
/// fewest broadcast-compliant sub-captions. Clean captions pass through.
#[tauri::command]
pub fn reflow_repair(project: Project, cfg: ReflowConfig) -> AppResult<Project> {
    reflow::repair(&project, &cfg, now_ms(), |_| new_id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Project, Style, Word};

    fn proj(captions: Vec<Caption>) -> Project {
        Project {
            id: "p".into(),
            name: "t".into(),
            video_path: "/x".into(),
            video_content_hash: "h".into(),
            video_duration_ms: 600_000,
            video_width: 1920,
            video_height: 1080,
            video_fps: 30.0,
            audio_wav_path: None,
            language: "en".into(),
            default_style: Style::broadcast_news(),
            context_description: None,
            captions,
            speakers: vec![],
            glossary: vec![],
            clips: vec![],
            talk_summary: None,
            export_config: crate::model::ExportConfig::default(),
            project_meta: crate::model::ProjectMeta::default(),
            created_at: 0,
            updated_at: 0,
            media: vec![],
            tracks: vec![],
            timeline_items: vec![],
        }
    }

    fn caption(id: &str, start: i64, end: i64, words: Vec<Word>) -> Caption {
        Caption {
            id: id.into(),
            start_ms: start,
            end_ms: end,
            words,
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
            track_id: None,
        }
    }

    #[test]
    fn analyze_command_forwards_to_service() {
        // A flashing caption (300ms) should be reported via the command.
        let c = caption("flash", 0, 300, vec![Word::new("hi", 0, 300, 90.0)]);
        let issues = reflow_analyze(proj(vec![c]), ReflowConfig::default()).unwrap();
        assert!(issues.iter().any(|i| i.kind == "min_duration"));
    }

    #[test]
    fn repair_command_splits_and_assigns_unique_ids() {
        let cfg = ReflowConfig {
            max_chars_per_line: 5,
            max_lines: 2,
            ..ReflowConfig::default()
        };
        let c = caption(
            "many",
            0,
            8000,
            (0..8)
                .map(|i| Word::new("alpha", i * 1000, i * 1000 + 900, 90.0))
                .collect(),
        );
        let out = reflow_repair(proj(vec![c]), cfg.clone()).unwrap();
        assert!(out.captions.len() > 1, "must split the long caption");
        // Real UUIDs are unique per sub-caption.
        let ids: std::collections::HashSet<&str> =
            out.captions.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), out.captions.len());
        assert!(reflow::analyze(&out, &cfg).is_empty());
    }
}
