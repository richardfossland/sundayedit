//! AI smart suggestions — Phase 4.3.
//!
//! Where polish (4.1) fixes only punctuation/casing and applies itself
//! behind a guard, Smart Suggest proposes *substantive* rewrites — fixing
//! mis-transcriptions from context, tightening run-on captions, shortening
//! for subtitle readability. Because these change wording, the plan's hard
//! constraint applies:
//!
//!   **The AI must NEVER silently change wording. Every substantive change
//!   requires explicit user approval.**
//!
//! This module honours that structurally: `suggest_captions` only ever
//! *returns a review queue* of [`Suggestion`]s. Nothing is mutated until
//! the user accepts one and the caller invokes [`apply_suggestion`] on that
//! single item.
//!
//! Caption display timing (start/end of the caption) is preserved on apply;
//! word-level timings are re-derived proportionally across that span, since
//! a rewrite has no per-word audio alignment.
//!
//! Pure pipeline (tested offline): build prompts → [network] →
//! parse_suggestions_response → apply_suggestion.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::{Project, Word};
use crate::services::llm::rough_token_count;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../src/lib/bindings/SuggestionKind.ts")]
pub enum SuggestionKind {
    /// Correct an obvious mis-transcription using surrounding context.
    FixTranscription,
    /// Tighten a run-on or unclear caption while keeping the meaning.
    Rephrase,
    /// Reduce reading load for on-screen display.
    Shorten,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../src/lib/bindings/Strictness.ts")]
pub enum Strictness {
    Conservative,
    Balanced,
    Aggressive,
}

impl Default for Strictness {
    fn default() -> Self {
        Strictness::Balanced
    }
}

impl Strictness {
    fn guidance(&self) -> &'static str {
        match self {
            Strictness::Conservative => {
                "Be very conservative: only propose the most clear-cut fixes, and leave anything \
                 borderline untouched."
            }
            Strictness::Balanced => {
                "Be balanced: propose a change when it is a clear improvement, but do not nitpick."
            }
            Strictness::Aggressive => {
                "Be thorough: propose readability rewrites wherever they would genuinely help, \
                 while still never altering meaning."
            }
        }
    }
}

/// One proposed change, returned to the UI as a review-queue item. Applied
/// only if the user accepts it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Suggestion.ts")]
pub struct Suggestion {
    /// Accept "id" too — some models prefer the shorter key.
    #[serde(alias = "id")]
    pub caption_id: String,
    pub kind: SuggestionKind,
    /// The full proposed replacement text for the caption.
    pub suggestion: String,
    /// One short sentence on why.
    pub reasoning: String,
}

#[derive(Serialize)]
struct SuggestInput<'a> {
    caption_id: &'a str,
    text: String,
}

// ── Building the request ──────────────────────────────────────────────────────

fn caption_inputs(project: &Project) -> Vec<SuggestInput<'_>> {
    project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| SuggestInput { caption_id: &c.id, text: c.text() })
        .collect()
}

fn language_name(language: &str) -> &'static str {
    match language {
        "no" | "nb" | "nn" => "Norwegian",
        "en" => "English",
        "sv" => "Swedish",
        "da" => "Danish",
        "de" => "German",
        _ => "the caption language",
    }
}

pub fn build_suggest_system_prompt(language: &str, strictness: Strictness) -> String {
    format!(
        "You are an expert subtitle editor working in {lang}. Propose improvements only to \
         captions that genuinely need them.\n\n\
         You may propose these kinds of change:\n\
         - \"fix-transcription\": correct an obvious mis-transcription using context.\n\
         - \"rephrase\": tighten a run-on or unclear caption while keeping the meaning.\n\
         - \"shorten\": reduce reading load for on-screen display.\n\n\
         Aim for subtitle conventions: about 42 characters per line, at most two lines, breaking \
         at natural phrase boundaries.\n\n\
         {strictness} Never propose a change that alters the speaker's meaning. When in doubt, \
         omit the caption.\n\n\
         Output ONLY a JSON array, with no prose and no code fences. Each element is an object \
         {{\"caption_id\": <the caption's id unchanged>, \"kind\": <one of \
         fix-transcription|rephrase|shorten>, \"suggestion\": <the full proposed replacement \
         text>, \"reasoning\": <one short sentence>}}. Omit every caption you would leave as-is.",
        lang = language_name(language),
        strictness = strictness.guidance(),
    )
}

pub fn build_suggest_user_prompt(project: &Project) -> String {
    let inputs = caption_inputs(project);
    let json = serde_json::to_string_pretty(&inputs).unwrap_or_else(|_| "[]".to_string());
    format!("Review these captions and suggest improvements where warranted:\n\n{json}")
}

/// Rough output-token estimate for the cost preview: suggestions plus
/// reasoning, only for some fraction of captions.
pub fn estimate_output_tokens(project: &Project) -> usize {
    let body: usize = project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| rough_token_count(&c.text()) * 2 + 16)
        .sum();
    ((body as f64) * 0.7).ceil() as usize + 64
}

// ── Parsing the response ──────────────────────────────────────────────────────

fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    (end > start).then(|| &s[start..=end])
}

pub fn parse_suggestions_response(response: &str) -> AppResult<Vec<Suggestion>> {
    let slice = extract_json_array(response)
        .ok_or_else(|| AppError::Validation("suggestion response had no JSON array".into()))?;
    let items: Vec<Suggestion> = serde_json::from_str(slice)
        .map_err(|e| AppError::Validation(format!("suggestion response was not valid JSON: {e}")))?;
    // Drop empty suggestions defensively — apply would reject them anyway.
    Ok(items.into_iter().filter(|s| !s.suggestion.trim().is_empty()).collect())
}

// ── Applying one accepted suggestion ──────────────────────────────────────────

/// Distribute `[start_ms, end_ms]` across `texts` proportional to each
/// token's length, so a rewrite gets plausible word timings within the
/// caption's preserved span. Monotonic; the last word ends exactly at
/// `end_ms`. New words are flagged `edited` (a user-approved content change)
/// at full confidence.
fn retime_words(texts: &[&str], start_ms: i64, end_ms: i64) -> Vec<Word> {
    let span = (end_ms - start_ms).max(1);
    let lens: Vec<i64> = texts.iter().map(|t| t.chars().count().max(1) as i64).collect();
    let total: i64 = lens.iter().sum::<i64>().max(1);

    let mut words = Vec::with_capacity(texts.len());
    let mut prev = start_ms;
    let mut acc = 0i64;
    let last = texts.len().saturating_sub(1);
    for (i, t) in texts.iter().enumerate() {
        acc += lens[i];
        let boundary = if i == last { end_ms } else { start_ms + span * acc / total };
        let end = boundary.max(prev + 1).min(end_ms);
        let mut w = Word::new(*t, prev, end.max(prev + 1), 100.0);
        w.edited = true;
        words.push(w);
        prev = end;
    }
    words
}

/// Apply a single accepted suggestion. Replaces the target caption's words
/// with the proposed text (re-timed within the caption span). Errors if the
/// caption is gone or the suggestion is empty.
pub fn apply_suggestion(project: &Project, suggestion: &Suggestion, now_ms: i64) -> AppResult<Project> {
    let text = suggestion.suggestion.trim();
    if text.is_empty() {
        return Err(AppError::Validation("suggestion text is empty".into()));
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(AppError::Validation("suggestion has no words".into()));
    }

    let mut next = project.clone();
    let cap = next
        .captions
        .iter_mut()
        .find(|c| c.id == suggestion.caption_id)
        .ok_or_else(|| AppError::NotFound { entity: "caption", id: suggestion.caption_id.clone() })?;

    cap.words = retime_words(&tokens, cap.start_ms, cap.end_ms);
    cap.ai_generated = false; // it's now a human-approved edit
    cap.last_edited_at = now_ms;
    next.updated_at = now_ms;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Style, Word};

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
        }
    }

    fn project_with(captions: Vec<Caption>) -> Project {
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
            glossary: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    fn sug(id: &str, kind: SuggestionKind, text: &str) -> Suggestion {
        Suggestion { caption_id: id.into(), kind, suggestion: text.into(), reasoning: "r".into() }
    }

    // ── prompts ──────────────────────────────────────────────────────────────
    #[test]
    fn system_prompt_reflects_language_and_strictness() {
        let p = build_suggest_system_prompt("no", Strictness::Conservative);
        assert!(p.contains("Norwegian"));
        assert!(p.contains("very conservative"));
        assert!(p.contains("NEVER") || p.contains("Never propose"));
    }

    #[test]
    fn user_prompt_lists_captions() {
        let p = project_with(vec![caption("c1", 0, 1000, vec![Word::new("hei", 0, 1000, 80.0)])]);
        let prompt = build_suggest_user_prompt(&p);
        assert!(prompt.contains("caption_id"));
        assert!(prompt.contains("c1"));
        assert!(prompt.contains("hei"));
    }

    // ── parsing ────────────────────────────────────────────────────────────────
    #[test]
    fn parses_suggestions_with_fences() {
        let r = "```json\n[{\"caption_id\":\"c1\",\"kind\":\"rephrase\",\"suggestion\":\"Hello there.\",\"reasoning\":\"clearer\"}]\n```";
        let s = parse_suggestions_response(r).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].caption_id, "c1");
        assert_eq!(s[0].kind, SuggestionKind::Rephrase);
        assert_eq!(s[0].suggestion, "Hello there.");
    }

    #[test]
    fn parser_accepts_id_alias() {
        let r = r#"[{"id":"c9","kind":"fix-transcription","suggestion":"kerygma","reasoning":"term"}]"#;
        let s = parse_suggestions_response(r).unwrap();
        assert_eq!(s[0].caption_id, "c9");
        assert_eq!(s[0].kind, SuggestionKind::FixTranscription);
    }

    #[test]
    fn parser_drops_empty_suggestions() {
        let r = r#"[{"caption_id":"c1","kind":"shorten","suggestion":"   ","reasoning":"x"}]"#;
        assert!(parse_suggestions_response(r).unwrap().is_empty());
    }

    #[test]
    fn parser_rejects_non_json() {
        assert!(parse_suggestions_response("no array here").is_err());
    }

    // ── apply ────────────────────────────────────────────────────────────────
    #[test]
    fn apply_replaces_words_and_preserves_caption_span() {
        let p = project_with(vec![caption(
            "c1",
            1000,
            5000,
            vec![
                Word::new("so", 1000, 2000, 60.0),
                Word::new("we", 2000, 3000, 65.0),
                Word::new("go", 3000, 5000, 70.0),
            ],
        )]);
        let s = sug("c1", SuggestionKind::Rephrase, "So we should go.");
        let out = apply_suggestion(&p, &s, 999).unwrap();

        let cap = &out.captions[0];
        // caption display span unchanged
        assert_eq!(cap.start_ms, 1000);
        assert_eq!(cap.end_ms, 5000);
        // words replaced with the proposal
        let texts: Vec<&str> = cap.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["So", "we", "should", "go."]);
        // first word starts at caption start, last ends at caption end
        assert_eq!(cap.words.first().unwrap().start_ms, 1000);
        assert_eq!(cap.words.last().unwrap().end_ms, 5000);
        // all marked edited, full confidence
        assert!(cap.words.iter().all(|w| w.edited && w.confidence == 100.0));
        assert_eq!(out.updated_at, 999);
        // the project must still satisfy its timing invariants
        out.validate().unwrap();
    }

    #[test]
    fn apply_word_timings_are_monotonic_and_in_bounds() {
        let p = project_with(vec![caption("c1", 0, 1000, vec![Word::new("a", 0, 1000, 50.0)])]);
        let s = sug("c1", SuggestionKind::Rephrase, "one two three four five six seven");
        let out = apply_suggestion(&p, &s, 1).unwrap();
        let cap = &out.captions[0];
        let mut prev = cap.start_ms;
        for w in &cap.words {
            assert!(w.start_ms >= prev, "non-monotonic start");
            assert!(w.end_ms > w.start_ms, "zero/negative duration");
            assert!(w.end_ms <= cap.end_ms, "word past caption end");
            prev = w.end_ms;
        }
        out.validate().unwrap();
    }

    #[test]
    fn apply_unknown_caption_errors() {
        let p = project_with(vec![caption("c1", 0, 1000, vec![Word::new("hi", 0, 1000, 90.0)])]);
        let s = sug("nope", SuggestionKind::Rephrase, "Hello.");
        let err = apply_suggestion(&p, &s, 1).unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn apply_empty_suggestion_errors() {
        let p = project_with(vec![caption("c1", 0, 1000, vec![Word::new("hi", 0, 1000, 90.0)])]);
        let s = sug("c1", SuggestionKind::Rephrase, "   ");
        assert!(apply_suggestion(&p, &s, 1).is_err());
    }

    #[test]
    fn applying_one_leaves_other_captions_untouched() {
        let p = project_with(vec![
            caption("c1", 0, 1000, vec![Word::new("hei", 0, 1000, 80.0)]),
            caption("c2", 2000, 4000, vec![Word::new("verden", 2000, 4000, 80.0)]),
        ]);
        let out = apply_suggestion(&p, &sug("c1", SuggestionKind::Rephrase, "Hei!"), 5).unwrap();
        assert_eq!(out.captions[1].words[0].text, "verden");
        assert!(!out.captions[1].words[0].edited);
    }
}
