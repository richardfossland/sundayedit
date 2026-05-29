//! AI caption translation — Phase 7.1.
//!
//! Translate a caption track to another language for international
//! audiences. The promise: meaning preserved, **timing preserved** — each
//! caption's display span is untouched and the translated text is re-timed
//! proportionally across it (a translation has no per-word audio
//! alignment). Glossary terms (Phase 3.4) are passed to the model so proper
//! nouns and jargon translate consistently.
//!
//! Translation is non-destructive at this layer: `translate_to_captions`
//! returns a [`TranslationResult`] (new caption list + length warnings)
//! without mutating the project. The caller decides whether to replace the
//! track. Captions the model omits keep their original words, so nothing is
//! lost.
//!
//! Pure pipeline (tested offline): supported_languages / build prompts →
//! [network] → parse_translation_response → translate_to_captions.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::{Caption, GlossaryTerm, Project, Word};
use crate::services::llm::rough_token_count;

/// A language offered in the target picker. Claude handles many more; this
/// is just a convenient curated list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/TranslationLanguage.ts")]
pub struct TranslationLanguage {
    pub code: String,
    pub name: String,
}

/// Flagged when a translated caption is much longer than its source, so
/// the reading speed may be too high for its display span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/LengthWarning.ts")]
pub struct LengthWarning {
    pub caption_id: String,
    pub original_chars: usize,
    pub translated_chars: usize,
}

/// Result of a translation pass: the translated caption track (timings
/// preserved), the target language, and any reading-speed warnings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/TranslationResult.ts")]
pub struct TranslationResult {
    pub target_language: String,
    pub captions: Vec<Caption>,
    pub warnings: Vec<LengthWarning>,
}

/// What the model returns / we parse: one translated caption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslatedCaption {
    #[serde(alias = "id")]
    pub caption_id: String,
    pub text: String,
}

#[derive(Serialize)]
struct TranslateInput<'a> {
    caption_id: &'a str,
    text: String,
}

/// A caption whose translation is longer than this multiple of the source
/// is flagged — at that point the same display span means noticeably faster
/// reading.
const LENGTH_WARN_RATIO: f64 = 1.5;

pub fn supported_languages() -> Vec<TranslationLanguage> {
    const L: &[(&str, &str)] = &[
        ("en", "English"),
        ("no", "Norsk"),
        ("sv", "Svenska"),
        ("da", "Dansk"),
        ("de", "Deutsch"),
        ("nl", "Nederlands"),
        ("fr", "Français"),
        ("es", "Español"),
        ("pt", "Português"),
        ("it", "Italiano"),
        ("pl", "Polski"),
        ("fi", "Suomi"),
        ("is", "Íslenska"),
        ("ru", "Русский"),
        ("uk", "Українська"),
        ("ar", "العربية"),
        ("zh", "中文"),
        ("ja", "日本語"),
        ("ko", "한국어"),
        ("hi", "हिन्दी"),
    ];
    L.iter()
        .map(|(c, n)| TranslationLanguage {
            code: c.to_string(),
            name: n.to_string(),
        })
        .collect()
}

pub fn language_name(code: &str) -> String {
    supported_languages()
        .into_iter()
        .find(|l| l.code == code)
        .map(|l| l.name)
        .unwrap_or_else(|| code.to_string())
}

// ── Building the request ──────────────────────────────────────────────────────

fn caption_inputs(project: &Project) -> Vec<TranslateInput<'_>> {
    project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| TranslateInput {
            caption_id: &c.id,
            text: c.text(),
        })
        .collect()
}

pub fn build_translate_system_prompt(target_code: &str, glossary: &[GlossaryTerm]) -> String {
    let target = language_name(target_code);
    let mut s = format!(
        "You are an expert subtitle translator. Translate each caption into {target}, preserving \
         the speaker's meaning and tone.\n\n\
         Rules:\n\
         - Keep translations concise — they must fit the same on-screen time as the source, so \
         favour natural, compact phrasing over literal word-for-word translation.\n\
         - Translate the meaning of each caption independently but consistently across the set.\n\
         - Do not add commentary, notes, or transliteration.\n",
    );
    if !glossary.is_empty() {
        let terms: Vec<&str> = glossary.iter().map(|t| t.term.as_str()).take(64).collect();
        s.push_str(&format!(
            "- Translate these names/terms consistently (keep proper nouns intact unless they have \
             an established {target} form): {}.\n",
            terms.join(", ")
        ));
    }
    s.push_str(
        "\nOutput ONLY a JSON array, with no prose and no code fences. Each element is an object \
         {\"caption_id\": <the caption's id unchanged>, \"text\": <the translation>}. Return every \
         caption you were given, with the same ids.",
    );
    s
}

pub fn build_translate_user_prompt(project: &Project) -> String {
    let inputs = caption_inputs(project);
    let json = serde_json::to_string_pretty(&inputs).unwrap_or_else(|_| "[]".to_string());
    format!("Translate these captions:\n\n{json}")
}

pub fn estimate_output_tokens(project: &Project) -> usize {
    let body: usize = project
        .captions
        .iter()
        .filter(|c| !c.words.is_empty())
        .map(|c| rough_token_count(&c.text()) + rough_token_count(&c.id) + 8)
        .sum();
    // Translations can run longer than source — budget generously.
    ((body as f64) * 1.6).ceil() as usize + 64
}

// ── Parsing ────────────────────────────────────────────────────────────────────

fn extract_json_array(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    (end > start).then(|| &s[start..=end])
}

pub fn parse_translation_response(response: &str) -> AppResult<Vec<TranslatedCaption>> {
    let slice = extract_json_array(response)
        .ok_or_else(|| AppError::Validation("translation response had no JSON array".into()))?;
    let items: Vec<TranslatedCaption> = serde_json::from_str(slice).map_err(|e| {
        AppError::Validation(format!("translation response was not valid JSON: {e}"))
    })?;
    Ok(items)
}

// ── Applying ────────────────────────────────────────────────────────────────────

/// Distribute `[start_ms, end_ms]` across `texts` proportional to length.
/// Monotonic; last word ends exactly at `end_ms`; words marked `edited`.
fn retime_words(texts: &[&str], start_ms: i64, end_ms: i64) -> Vec<Word> {
    let span = (end_ms - start_ms).max(1);
    let lens: Vec<i64> = texts
        .iter()
        .map(|t| t.chars().count().max(1) as i64)
        .collect();
    let total: i64 = lens.iter().sum::<i64>().max(1);

    let mut words = Vec::with_capacity(texts.len());
    let mut prev = start_ms;
    let mut acc = 0i64;
    let last = texts.len().saturating_sub(1);
    for (i, t) in texts.iter().enumerate() {
        acc += lens[i];
        let boundary = if i == last {
            end_ms
        } else {
            start_ms + span * acc / total
        };
        let end = boundary.max(prev + 1).min(end_ms);
        let mut w = Word::new(*t, prev, end.max(prev + 1), 100.0);
        w.edited = true;
        words.push(w);
        prev = end;
    }
    words
}

/// Build the translated caption track. Each caption keeps its span; its
/// words become the translation, re-timed within that span. Captions the
/// model omitted keep their original words. Reading-speed warnings are
/// collected where the translation is much longer than the source.
pub fn translate_to_captions(
    project: &Project,
    translated: &[TranslatedCaption],
    target_code: &str,
    now_ms: i64,
) -> TranslationResult {
    use std::collections::HashMap;
    let by_id: HashMap<&str, &str> = translated
        .iter()
        .map(|t| (t.caption_id.as_str(), t.text.as_str()))
        .collect();

    let mut out = Vec::with_capacity(project.captions.len());
    let mut warnings = Vec::new();

    for cap in &project.captions {
        let mut next = cap.clone();
        if let Some(text) = by_id
            .get(cap.id.as_str())
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
        {
            let tokens: Vec<&str> = text.split_whitespace().collect();
            if !tokens.is_empty() {
                let original_chars = cap.text().chars().count();
                let translated_chars = text.chars().count();
                if original_chars > 0
                    && (translated_chars as f64) > (original_chars as f64) * LENGTH_WARN_RATIO
                {
                    warnings.push(LengthWarning {
                        caption_id: cap.id.clone(),
                        original_chars,
                        translated_chars,
                    });
                }
                next.words = retime_words(&tokens, cap.start_ms, cap.end_ms);
                next.ai_generated = false;
                next.last_edited_at = now_ms;
            }
        }
        out.push(next);
    }

    TranslationResult {
        target_language: target_code.to_string(),
        captions: out,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, GlossaryTerm, Style, Word};

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
            created_at: 0,
            updated_at: 0,
        }
    }

    fn tc(id: &str, text: &str) -> TranslatedCaption {
        TranslatedCaption {
            caption_id: id.into(),
            text: text.into(),
        }
    }

    #[test]
    fn supported_languages_are_unique_and_nonempty() {
        let langs = supported_languages();
        assert!(langs.len() >= 15);
        let mut codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
        codes.sort();
        let n = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), n, "duplicate language codes");
    }

    #[test]
    fn system_prompt_names_target_and_glossary() {
        let g = vec![GlossaryTerm {
            id: "g1".into(),
            term: "kerygma".into(),
            aliases: vec![],
            definition: None,
            pronunciation_hint: None,
        }];
        let p = build_translate_system_prompt("en", &g);
        assert!(p.contains("English"));
        assert!(p.contains("kerygma"));
        assert!(p.contains("JSON array"));
    }

    #[test]
    fn user_prompt_lists_captions() {
        let p = project_with(
            vec![caption(
                "c1",
                0,
                1000,
                vec![Word::new("hei", 0, 1000, 80.0)],
            )],
            vec![],
        );
        let prompt = build_translate_user_prompt(&p);
        assert!(prompt.contains("c1"));
        assert!(prompt.contains("hei"));
    }

    #[test]
    fn parses_translation_with_fences_and_id_alias() {
        let r = "```json\n[{\"id\":\"c1\",\"text\":\"Hello world\"}]\n```";
        let t = parse_translation_response(r).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].caption_id, "c1");
        assert_eq!(t[0].text, "Hello world");
    }

    #[test]
    fn parse_rejects_non_json() {
        assert!(parse_translation_response("nope").is_err());
    }

    #[test]
    fn applies_translation_preserving_span_and_timings() {
        let p = project_with(
            vec![caption(
                "c1",
                1000,
                4000,
                vec![
                    Word::new("Hei", 1000, 2500, 90.0),
                    Word::new("verden", 2500, 4000, 90.0),
                ],
            )],
            vec![],
        );
        let res = translate_to_captions(&p, &[tc("c1", "Hello world")], "en", 500);
        let cap = &res.captions[0];
        assert_eq!(res.target_language, "en");
        assert_eq!(cap.start_ms, 1000);
        assert_eq!(cap.end_ms, 4000);
        let texts: Vec<&str> = cap.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, vec!["Hello", "world"]);
        assert_eq!(cap.words.first().unwrap().start_ms, 1000);
        assert_eq!(cap.words.last().unwrap().end_ms, 4000);
        assert!(cap.words.iter().all(|w| w.edited));
        // build a project from the result and confirm invariants hold
        let mut translated_project = p.clone();
        translated_project.captions = res.captions.clone();
        translated_project.validate().unwrap();
    }

    #[test]
    fn omitted_caption_keeps_original() {
        let p = project_with(
            vec![
                caption("c1", 0, 1000, vec![Word::new("hei", 0, 1000, 80.0)]),
                caption(
                    "c2",
                    2000,
                    3000,
                    vec![Word::new("verden", 2000, 3000, 80.0)],
                ),
            ],
            vec![],
        );
        // Only c1 translated.
        let res = translate_to_captions(&p, &[tc("c1", "hi")], "en", 1);
        assert_eq!(res.captions[0].words[0].text, "hi");
        assert_eq!(
            res.captions[1].words[0].text, "verden",
            "untranslated caption preserved"
        );
        assert!(!res.captions[1].words[0].edited);
    }

    #[test]
    fn flags_overlong_translation() {
        // Source "ja" (2 chars) → a long German translation triggers warning.
        let p = project_with(
            vec![caption("c1", 0, 2000, vec![Word::new("ja", 0, 2000, 90.0)])],
            vec![],
        );
        let long = "selbstverständlich auf jeden Fall ganz bestimmt";
        let res = translate_to_captions(&p, &[tc("c1", long)], "de", 1);
        assert_eq!(res.warnings.len(), 1);
        assert_eq!(res.warnings[0].caption_id, "c1");
        assert!(res.warnings[0].translated_chars > res.warnings[0].original_chars);
    }

    #[test]
    fn similar_length_translation_has_no_warning() {
        let p = project_with(
            vec![caption(
                "c1",
                0,
                2000,
                vec![Word::new("hello", 0, 2000, 90.0)],
            )],
            vec![],
        );
        let res = translate_to_captions(&p, &[tc("c1", "hallo")], "de", 1);
        assert!(res.warnings.is_empty());
    }

    #[test]
    fn empty_translation_text_keeps_original() {
        let p = project_with(
            vec![caption(
                "c1",
                0,
                1000,
                vec![Word::new("hei", 0, 1000, 80.0)],
            )],
            vec![],
        );
        let res = translate_to_captions(&p, &[tc("c1", "   ")], "en", 1);
        assert_eq!(res.captions[0].words[0].text, "hei");
    }
}
