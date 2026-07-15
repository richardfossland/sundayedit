//! AI-suggested glossary terms — Phase 3.4 (killer feature #2, mode 3).
//!
//! After a first-pass transcript, scan it for the words a speech recognizer
//! is most likely to get wrong — proper nouns, jargon, foreign words — and
//! propose them as glossary entries (canonical term + likely misrecognitions
//! to auto-correct). Like Smart Suggest, this is propose-and-approve: the
//! command only ever *returns* candidates; the UI adds the ones the user
//! accepts into `project.glossary`.
//!
//! Pure pipeline (tested offline): build prompts → [network] →
//! parse_glossary_suggestions.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::Project;
use crate::services::llm::rough_token_count;

/// A proposed glossary entry, returned to the UI for review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/SuggestedTerm.ts")]
pub struct SuggestedTerm {
    /// Canonical spelling we want in the output.
    pub term: String,
    /// Likely misrecognitions to auto-correct to `term`.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// One short sentence on why it's worth a glossary entry.
    #[serde(default)]
    pub reason: String,
}

fn language_name(language: &str) -> &'static str {
    match language {
        "no" | "nb" | "nn" => "Norwegian",
        "en" => "English",
        "sv" => "Swedish",
        "da" => "Danish",
        "de" => "German",
        _ => "the transcript language",
    }
}

/// The full transcript text, captions joined in order.
fn transcript_text(project: &Project) -> String {
    project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| c.text())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn build_glossary_system_prompt(language: &str) -> String {
    format!(
        "You are helping build a glossary that improves speech-to-text accuracy for a recording \
         in {lang}. Read the transcript and identify the terms a speech recognizer is most likely \
         to get wrong: proper nouns (people, places, organizations), technical or domain jargon, \
         and foreign words. For each, give the correct canonical spelling, plausible \
         misrecognitions a recognizer might produce instead (aliases), and one short reason.\n\n\
         Ignore ordinary vocabulary and anything already spelled correctly and common. Do NOT \
         propose terms that are already in the existing glossary you are given.\n\n\
         Output ONLY a JSON array, with no prose and no code fences. Each element is \
         {{\"term\": <canonical spelling>, \"aliases\": [<likely misrecognitions>], \"reason\": \
         <one short sentence>}}. Return an empty array [] if nothing qualifies.",
        lang = language_name(language),
    )
}

pub fn build_glossary_user_prompt(project: &Project) -> String {
    let existing: Vec<&str> = project.glossary.iter().map(|t| t.term.as_str()).collect();
    let existing_line = if existing.is_empty() {
        "(none yet)".to_string()
    } else {
        existing.join(", ")
    };
    format!(
        "Existing glossary terms (do not repeat these): {existing}\n\n\
         Transcript:\n{body}",
        existing = existing_line,
        body = transcript_text(project),
    )
}

/// Rough output-token estimate for the cost preview. Glossaries are short, so
/// scale gently with transcript size and floor it.
pub fn estimate_output_tokens(project: &Project) -> usize {
    let transcript = rough_token_count(&transcript_text(project));
    (transcript / 12).clamp(48, 1024) + 32
}

// ── Mode 4: terms from a reference document (pre-transcription) ───────────────
//
// Same propose-and-approve contract as mode 3 — these only build prompts; the
// response is parsed by `parse_glossary_suggestions` and reviewed by the user.
// The difference is the source: a document the speaker is working from, fed in
// *before* a transcript exists, so Whisper can be primed on its vocabulary.

pub fn build_doc_glossary_system_prompt(language: &str) -> String {
    format!(
        "You are helping build a glossary that improves speech-to-text accuracy for an upcoming \
         recording in {lang}. You are given a REFERENCE DOCUMENT the speaker is working from — a \
         script, manuscript, lecture notes, or article. From it, identify the terms a speech \
         recognizer is most likely to get wrong: proper nouns (people, places, organizations), \
         technical or domain jargon, and foreign words. For each, give the correct canonical \
         spelling, plausible misrecognitions a recognizer might produce instead (aliases), and \
         one short reason.\n\n\
         Ignore ordinary vocabulary and anything common and easy to recognize. Do NOT propose \
         terms that are already in the existing glossary you are given.\n\n\
         Output ONLY a JSON array, with no prose and no code fences. Each element is \
         {{\"term\": <canonical spelling>, \"aliases\": [<likely misrecognitions>], \"reason\": \
         <one short sentence>}}. Return an empty array [] if nothing qualifies.",
        lang = language_name(language),
    )
}

pub fn build_doc_glossary_user_prompt(project: &Project, document: &str) -> String {
    let existing: Vec<&str> = project.glossary.iter().map(|t| t.term.as_str()).collect();
    let existing_line = if existing.is_empty() {
        "(none yet)".to_string()
    } else {
        existing.join(", ")
    };
    format!(
        "Existing glossary terms (do not repeat these): {existing}\n\n\
         Reference document:\n{body}",
        existing = existing_line,
        body = document.trim(),
    )
}

/// Output-token estimate for a document pass. The candidate list scales with
/// how much unique vocabulary the document holds, so scale gently with its
/// size and floor it.
pub fn estimate_doc_output_tokens(document: &str) -> usize {
    let doc = rough_token_count(document);
    (doc / 10).clamp(48, 1024) + 32
}

fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    (end > start).then(|| &s[start..=end])
}

pub fn parse_glossary_suggestions(response: &str) -> AppResult<Vec<SuggestedTerm>> {
    let slice = extract_json_array(response)
        .ok_or_else(|| AppError::Validation("glossary response had no JSON array".into()))?;
    let items: Vec<SuggestedTerm> = serde_json::from_str(slice)
        .map_err(|e| AppError::Validation(format!("glossary response was not valid JSON: {e}")))?;
    // Defensive: drop blank terms and trim; dedupe by canonical term.
    let mut seen = std::collections::HashSet::new();
    Ok(items
        .into_iter()
        .filter_map(|mut t| {
            t.term = t.term.trim().to_string();
            if t.term.is_empty() {
                return None;
            }
            // Dedupe aliases case-insensitively and drop any that merely echo
            // the canonical term — an LLM routinely repeats case variants and
            // the term itself, which would prime Whisper redundantly and bloat
            // the stored glossary on every accept.
            let term_lc = t.term.to_lowercase();
            let mut alias_seen = std::collections::HashSet::new();
            t.aliases = t
                .aliases
                .into_iter()
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty())
                .filter(|a| {
                    let lc = a.to_lowercase();
                    lc != term_lc && alias_seen.insert(lc)
                })
                .collect();
            seen.insert(t.term.to_lowercase()).then_some(t)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, GlossaryTerm, Style, Word};

    fn project_with(captions: Vec<Caption>, glossary: Vec<GlossaryTerm>) -> Project {
        Project {
            id: "p".into(),
            name: "t".into(),
            video_path: "/x".into(),
            video_content_hash: "h".into(),
            video_duration_ms: 60_000,
            video_width: 0,
            video_height: 0,
            video_fps: 0.0,
            audio_wav_path: None,
            language: "no".into(),
            default_style: Style::broadcast_news(),
            context_description: None,
            captions,
            speakers: vec![],
            glossary,
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

    fn caption(id: &str, text: &str) -> Caption {
        let words = text
            .split_whitespace()
            .enumerate()
            .map(|(i, w)| Word::new(w, i as i64 * 500, i as i64 * 500 + 500, 80.0))
            .collect();
        Caption {
            id: id.into(),
            start_ms: 0,
            end_ms: 1000,
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
    fn system_prompt_names_language() {
        let p = build_glossary_system_prompt("no");
        assert!(p.contains("Norwegian"));
        assert!(p.contains("JSON array"));
    }

    #[test]
    fn user_prompt_includes_transcript_and_existing_terms() {
        let p = project_with(
            vec![caption("c1", "Vi snakker om kerygma")],
            vec![GlossaryTerm {
                id: "g1".into(),
                term: "soteriologi".into(),
                aliases: vec![],
                definition: None,
                pronunciation_hint: None,
            }],
        );
        let prompt = build_glossary_user_prompt(&p);
        assert!(prompt.contains("kerygma"));
        assert!(prompt.contains("soteriologi")); // existing term listed
        assert!(prompt.contains("do not repeat"));
    }

    #[test]
    fn user_prompt_handles_empty_glossary() {
        let p = project_with(vec![caption("c1", "hei")], vec![]);
        assert!(build_glossary_user_prompt(&p).contains("(none yet)"));
    }

    #[test]
    fn parses_array_with_fences() {
        let r = "```json\n[{\"term\":\"kerygma\",\"aliases\":[\"kerigma\"],\"reason\":\"theological term\"}]\n```";
        let out = parse_glossary_suggestions(r).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].term, "kerygma");
        assert_eq!(out[0].aliases, vec!["kerigma"]);
    }

    #[test]
    fn drops_blank_terms_and_trims_aliases() {
        let r = r#"[{"term":"  ","aliases":["x"],"reason":"r"},{"term":" Lars ","aliases":["  ","Las"],"reason":"name"}]"#;
        let out = parse_glossary_suggestions(r).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].term, "Lars"); // trimmed
        assert_eq!(out[0].aliases, vec!["Las"]); // blank alias dropped
    }

    #[test]
    fn dedupes_by_canonical_term_case_insensitively() {
        let r = r#"[{"term":"Lars","aliases":[],"reason":"a"},{"term":"lars","aliases":[],"reason":"b"}]"#;
        let out = parse_glossary_suggestions(r).unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn missing_aliases_and_reason_default() {
        let r = r#"[{"term":"Maria"}]"#;
        let out = parse_glossary_suggestions(r).unwrap();
        assert_eq!(out[0].term, "Maria");
        assert!(out[0].aliases.is_empty());
        assert_eq!(out[0].reason, "");
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_glossary_suggestions("nothing here").is_err());
    }

    #[test]
    fn dedupes_aliases_within_a_term() {
        // An LLM routinely repeats an alias (case variants) and even echoes
        // the canonical spelling back as an "alias". Duplicates would prime
        // Whisper redundantly and bloat the stored glossary on every accept.
        let r = r#"[{"term":"kerygma","aliases":["kerigma","Kerigma","kerigma","kerygma"],"reason":"x"}]"#;
        let out = parse_glossary_suggestions(r).unwrap();
        assert_eq!(
            out[0].aliases,
            vec!["kerigma"],
            "case-insensitive duplicate aliases and the canonical echo must collapse"
        );
    }

    #[test]
    fn empty_array_is_ok() {
        assert!(parse_glossary_suggestions("[]").unwrap().is_empty());
    }

    #[test]
    fn doc_system_prompt_targets_a_reference_document() {
        let p = build_doc_glossary_system_prompt("no");
        assert!(p.contains("Norwegian"));
        assert!(p.contains("REFERENCE DOCUMENT"));
        assert!(p.contains("JSON array"));
    }

    #[test]
    fn doc_user_prompt_embeds_document_and_existing_terms() {
        let p = project_with(
            vec![],
            vec![GlossaryTerm {
                id: "g1".into(),
                term: "soteriologi".into(),
                aliases: vec![],
                definition: None,
                pronunciation_hint: None,
            }],
        );
        let prompt = build_doc_glossary_user_prompt(&p, "  Vi snakker om kerygma  ");
        assert!(prompt.contains("kerygma"));
        assert!(prompt.contains("soteriologi")); // existing term listed
        assert!(prompt.contains("do not repeat"));
        assert!(prompt.contains("Reference document:"));
    }

    #[test]
    fn doc_user_prompt_works_without_a_transcript() {
        // Mode 4 runs before transcription — empty captions must be fine.
        let p = project_with(vec![], vec![]);
        let prompt = build_doc_glossary_user_prompt(&p, "Bonhoeffer");
        assert!(prompt.contains("(none yet)"));
        assert!(prompt.contains("Bonhoeffer"));
    }

    #[test]
    fn doc_output_estimate_is_floored_and_capped() {
        assert_eq!(estimate_doc_output_tokens(""), 48 + 32); // floored
        let huge = "term ".repeat(100_000);
        assert_eq!(estimate_doc_output_tokens(&huge), 1024 + 32); // capped
    }
}
