//! AI social-clip planner — SundayEdit's headline feature.
//!
//! Given a transcribed talk, Claude proposes a handful of short, self-contained
//! social clips. Each clip references the SOURCE CAPTION IDS it covers (never
//! model-invented timestamps), so the real `start_ms`/`end_ms` are derived from
//! the captions themselves — the model can't drift the timeline. It also writes
//! a short summary of the whole talk.
//!
//! Mirrors `suggest.rs`: a pure pipeline (tested offline) of build prompts →
//! [network] → `parse_clips_response`. Nothing is applied until the user
//! reviews and the caller persists the plan via `clips_apply_plan`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::{Clip, Project};

/// The AI's proposed plan — a talk summary plus the reviewable clips.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ClipPlan.ts")]
pub struct ClipPlan {
    pub talk_summary: String,
    pub clips: Vec<Clip>,
}

#[derive(Serialize)]
struct ClipInput<'a> {
    caption_id: &'a str,
    start_ms: i64,
    end_ms: i64,
    text: String,
}

fn language_name(language: &str) -> &'static str {
    match language {
        "no" | "nb" | "nn" => "Norwegian",
        "en" => "English",
        "sv" => "Swedish",
        "da" => "Danish",
        "de" => "German",
        _ => "the talk's language",
    }
}

fn caption_inputs(project: &Project) -> Vec<ClipInput<'_>> {
    project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| ClipInput {
            caption_id: &c.id,
            start_ms: c.start_ms,
            end_ms: c.end_ms,
            text: c.text(),
        })
        .collect()
}

// ── Building the request ──────────────────────────────────────────────────────

pub fn build_clips_system_prompt(language: &str) -> String {
    format!(
        "You are a social-media producer for a church/teaching channel, working in {lang}. \
         You are given a full talk as a list of timed captions. Find the moments that work as \
         short, self-contained social clips (roughly 15–90 seconds each): a clear hook, one \
         complete point, a satisfying close. Aim for 3–8 clips — quality over quantity.\n\n\
         For each clip choose a CONTIGUOUS run of caption ids that together form the clip. Do NOT \
         invent timestamps — only reference caption ids exactly as given. For each clip also write \
         a short, punchy `title` (the main point, ≤ ~60 characters, no trailing period) that will \
         be shown as a large on-screen overlay, and a one-line `hook` summarising why the clip \
         lands.\n\n\
         Also write a `talk_summary`: 2–4 sentences capturing the whole talk's main points.\n\n\
         Output ONLY a JSON object, with no prose and no code fences:\n\
         {{\"talk_summary\": <string>, \"clips\": [{{\"caption_ids\": [<ids in order>], \
         \"title\": <string>, \"hook\": <string>}}]}}",
        lang = language_name(language),
    )
}

pub fn build_clips_user_prompt(project: &Project) -> String {
    let inputs = caption_inputs(project);
    let json = serde_json::to_string_pretty(&inputs).unwrap_or_else(|_| "[]".to_string());
    let ctx = project
        .context_description
        .as_deref()
        .map(|c| format!("Context for this talk: {c}\n\n"))
        .unwrap_or_default();
    format!("{ctx}Here is the talk as timed captions. Propose social clips:\n\n{json}")
}

/// Rough output-token estimate for the cost preview: a talk summary plus a
/// handful of small clip objects. Deterministic from the caption count.
pub fn estimate_output_tokens(project: &Project) -> usize {
    let caption_count = project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .count();
    let expected_clips = (caption_count / 6 + 1).clamp(3, 12);
    // summary (~160) + per-clip title/hook/ids (~70)
    160 + expected_clips * 70
}

// ── Parsing the response ──────────────────────────────────────────────────────

fn extract_json_object(s: &str) -> Option<&str> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| &s[start..=end])
}

#[derive(Deserialize)]
struct RawPlan {
    #[serde(default)]
    talk_summary: String,
    #[serde(default)]
    clips: Vec<RawClip>,
}

#[derive(Deserialize)]
struct RawClip {
    #[serde(default, alias = "captions")]
    caption_ids: Vec<String>,
    #[serde(default)]
    title: String,
    #[serde(default, alias = "summary")]
    hook: String,
}

/// Parse the model's plan and ground every clip in real caption timings.
/// Unknown caption ids are dropped; a clip with no resolvable ids or an empty
/// title is dropped. `start_ms`/`end_ms` come from the referenced captions
/// (clamped to the video), and clips are returned sorted by start.
pub fn parse_clips_response(project: &Project, response: &str) -> AppResult<ClipPlan> {
    let slice = extract_json_object(response)
        .ok_or_else(|| AppError::Validation("clip response had no JSON object".into()))?;
    let raw: RawPlan = serde_json::from_str(slice)
        .map_err(|e| AppError::Validation(format!("clip response was not valid JSON: {e}")))?;

    let bounds: HashMap<&str, (i64, i64)> = project
        .captions
        .iter()
        .map(|c| (c.id.as_str(), (c.start_ms, c.end_ms)))
        .collect();
    let duration = project.video_duration_ms.max(0);

    let mut clips: Vec<Clip> = Vec::new();
    for (i, rc) in raw.clips.into_iter().enumerate() {
        let title = rc.title.trim().to_string();
        if title.is_empty() {
            continue;
        }
        // Keep only ids that exist, preserving the model's order.
        let ids: Vec<String> = rc
            .caption_ids
            .into_iter()
            .filter(|id| bounds.contains_key(id.as_str()))
            .collect();
        if ids.is_empty() {
            continue;
        }
        let mut start = i64::MAX;
        let mut end = i64::MIN;
        for id in &ids {
            let (s, e) = bounds[id.as_str()];
            start = start.min(s);
            end = end.max(e);
        }
        start = start.max(0);
        if duration > 0 {
            end = end.min(duration);
        }
        if end <= start {
            continue;
        }
        clips.push(Clip {
            id: format!("clip:{i}"),
            title,
            hook: rc.hook.trim().to_string(),
            caption_ids: ids,
            start_ms: start,
            end_ms: end,
        });
    }
    clips.sort_by_key(|c| c.start_ms);

    Ok(ClipPlan {
        talk_summary: raw.talk_summary.trim().to_string(),
        clips,
    })
}

// ── Applying a reviewed plan ───────────────────────────────────────────────────

/// Persist a reviewed plan onto the project: replace its clips and talk
/// summary. The user curates `plan` (edits titles, drops clips) before this.
pub fn apply_plan(project: &Project, plan: &ClipPlan, now_ms: i64) -> Project {
    let mut next = project.clone();
    next.clips = plan.clips.clone();
    let summary = plan.talk_summary.trim();
    next.talk_summary = (!summary.is_empty()).then(|| summary.to_string());
    next.updated_at = now_ms;
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Style, Word};

    fn caption(id: &str, start: i64, end: i64) -> Caption {
        Caption {
            id: id.into(),
            start_ms: start,
            end_ms: end,
            words: vec![Word::new("ord", start, end, 90.0)],
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
            track_id: None,
        }
    }

    fn project_with(captions: Vec<Caption>, duration_ms: i64) -> Project {
        Project {
            id: "p".into(),
            name: "t".into(),
            video_path: "/x".into(),
            video_content_hash: "h".into(),
            video_duration_ms: duration_ms,
            video_width: 0,
            video_height: 0,
            video_fps: 0.0,
            audio_wav_path: None,
            language: "no".into(),
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

    fn sample() -> Project {
        project_with(
            vec![
                caption("c1", 0, 2000),
                caption("c2", 2000, 5000),
                caption("c3", 5000, 9000),
                caption("c4", 9000, 12000),
            ],
            12_000,
        )
    }

    #[test]
    fn system_prompt_reflects_language() {
        let p = build_clips_system_prompt("no");
        assert!(p.contains("Norwegian"));
        assert!(p.contains("caption_ids"));
        assert!(p.contains("talk_summary"));
    }

    #[test]
    fn user_prompt_lists_caption_ids_and_context() {
        let mut p = sample();
        p.context_description = Some("A sermon on grace".into());
        let prompt = build_clips_user_prompt(&p);
        assert!(prompt.contains("c1"));
        assert!(prompt.contains("A sermon on grace"));
    }

    #[test]
    fn derives_range_from_referenced_captions() {
        let r = r#"{"talk_summary":"A talk.","clips":[
            {"caption_ids":["c2","c3"],"title":"The main point","hook":"why it lands"}
        ]}"#;
        let plan = parse_clips_response(&sample(), r).unwrap();
        assert_eq!(plan.talk_summary, "A talk.");
        assert_eq!(plan.clips.len(), 1);
        let c = &plan.clips[0];
        assert_eq!(c.start_ms, 2000); // min of c2,c3
        assert_eq!(c.end_ms, 9000); // max of c2,c3
        assert_eq!(c.title, "The main point");
        assert_eq!(c.caption_ids, vec!["c2", "c3"]);
    }

    #[test]
    fn strips_code_fences() {
        let r = "```json\n{\"talk_summary\":\"s\",\"clips\":[{\"caption_ids\":[\"c1\"],\"title\":\"T\",\"hook\":\"h\"}]}\n```";
        let plan = parse_clips_response(&sample(), r).unwrap();
        assert_eq!(plan.clips.len(), 1);
    }

    #[test]
    fn drops_unknown_ids_and_empties() {
        let r = r#"{"talk_summary":"","clips":[
            {"caption_ids":["nope","c1"],"title":"Keep","hook":""},
            {"caption_ids":["nope"],"title":"Drop me","hook":""},
            {"caption_ids":["c2"],"title":"   ","hook":""}
        ]}"#;
        let plan = parse_clips_response(&sample(), r).unwrap();
        assert_eq!(plan.clips.len(), 1);
        assert_eq!(plan.clips[0].title, "Keep");
        assert_eq!(plan.clips[0].caption_ids, vec!["c1"]); // unknown filtered out
    }

    #[test]
    fn clamps_end_to_video_duration() {
        // c4 ends at 12000 == duration; a shorter duration must clamp.
        let p = project_with(
            vec![caption("c1", 0, 2000), caption("c2", 2000, 30_000)],
            10_000,
        );
        let r = r#"{"talk_summary":"s","clips":[{"caption_ids":["c2"],"title":"T","hook":"h"}]}"#;
        let plan = parse_clips_response(&p, r).unwrap();
        assert_eq!(plan.clips[0].end_ms, 10_000);
    }

    #[test]
    fn sorts_clips_by_start() {
        let r = r#"{"talk_summary":"s","clips":[
            {"caption_ids":["c4"],"title":"Late","hook":""},
            {"caption_ids":["c1"],"title":"Early","hook":""}
        ]}"#;
        let plan = parse_clips_response(&sample(), r).unwrap();
        assert_eq!(plan.clips[0].title, "Early");
        assert_eq!(plan.clips[1].title, "Late");
    }

    #[test]
    fn accepts_captions_alias() {
        let r = r#"{"clips":[{"captions":["c1"],"title":"T","hook":"h"}]}"#;
        let plan = parse_clips_response(&sample(), r).unwrap();
        assert_eq!(plan.clips.len(), 1);
        assert_eq!(plan.talk_summary, ""); // default when absent
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_clips_response(&sample(), "no object here").is_err());
    }

    #[test]
    fn apply_plan_sets_clips_and_summary() {
        let p = sample();
        let plan = ClipPlan {
            talk_summary: "  A talk.  ".into(),
            clips: vec![Clip {
                id: "clip:0".into(),
                title: "T".into(),
                hook: "h".into(),
                caption_ids: vec!["c1".into()],
                start_ms: 0,
                end_ms: 2000,
            }],
        };
        let out = apply_plan(&p, &plan, 42);
        assert_eq!(out.clips.len(), 1);
        assert_eq!(out.talk_summary.as_deref(), Some("A talk.")); // trimmed
        assert_eq!(out.updated_at, 42);
    }

    #[test]
    fn apply_plan_empty_summary_is_none() {
        let out = apply_plan(
            &sample(),
            &ClipPlan {
                talk_summary: "   ".into(),
                clips: vec![],
            },
            1,
        );
        assert_eq!(out.talk_summary, None);
        assert!(out.clips.is_empty());
    }

    #[test]
    fn estimate_scales_with_captions() {
        let small = estimate_output_tokens(&sample());
        let big = estimate_output_tokens(&project_with(
            (0..100)
                .map(|i| caption(&format!("c{i}"), i * 1000, i * 1000 + 900))
                .collect(),
            100_000,
        ));
        assert!(big > small);
    }
}
