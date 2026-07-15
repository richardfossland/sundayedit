//! Glossary auto-correction — Phase 3.4 (killer feature #2).
//!
//! Whisper mis-hears proper nouns, jargon, and foreign words. The
//! glossary fixes this in two ways:
//!   1. *Before* transcription: terms feed Whisper's `initial_prompt`
//!      (see `asr::AsrOptions::initial_prompt`) so recognition is biased
//!      toward them.
//!   2. *After* transcription: this module's post-pass scans every word
//!      and replaces known mis-spellings (aliases) with the canonical
//!      term — catching whatever priming missed.
//!
//! This is the *after* pass. It's a pure function (Project + glossary →
//! new Project + a list of corrections the user can review/undo) so it's
//! exhaustively testable.
//!
//! Matching rules:
//!   - Case-insensitive.
//!   - Leading/trailing punctuation on the transcript word is preserved
//!     ("kerigma," → "kerygma,").
//!   - A corrected word is marked `edited` so it loses its uncertainty
//!     highlight and isn't re-flagged.
//!   - We never touch a word the user has already edited or locked.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{GlossaryTerm, Project};

/// One applied correction — surfaced so the user can roll it back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/GlossaryCorrection.ts")]
pub struct GlossaryCorrection {
    pub caption_id: String,
    pub word_index: usize,
    pub from: String,
    pub to: String,
    pub term_id: String,
}

/// Apply the glossary to a project. Returns the updated project and the
/// list of corrections made.
pub fn apply_glossary(project: &Project, now_ms: i64) -> (Project, Vec<GlossaryCorrection>) {
    let mut next = project.clone();
    let mut corrections = Vec::new();

    // Build a lookup: normalized alias/term → (canonical term, term id).
    // Canonical terms also map to themselves so an already-correct word is
    // a no-op (and we don't accidentally "correct" it to a different case).
    let lookup = build_lookup(&project.glossary);
    if lookup.is_empty() {
        return (next, corrections);
    }

    for cap in next.captions.iter_mut() {
        let mut changed = false;
        for (wi, word) in cap.words.iter_mut().enumerate() {
            if word.edited || word.locked {
                continue; // respect human decisions
            }
            let (prefix, core, suffix) = split_affixes(&word.text);
            if core.is_empty() {
                continue;
            }
            let key = normalize(core);
            if let Some((canonical, term_id)) = lookup.get(&key) {
                // Only a correction if the canonical form differs from
                // what's there (case-insensitive identical → skip).
                if normalize(core) != normalize(canonical) || core != *canonical {
                    if core == canonical.as_str() {
                        continue; // already exactly correct
                    }
                    let new_text = format!("{}{}{}", prefix, canonical, suffix);
                    if new_text != word.text {
                        corrections.push(GlossaryCorrection {
                            caption_id: cap.id.clone(),
                            word_index: wi,
                            from: word.text.clone(),
                            to: new_text.clone(),
                            term_id: term_id.clone(),
                        });
                        word.text = new_text;
                        word.edited = true;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            cap.last_edited_at = now_ms;
        }
    }

    if !corrections.is_empty() {
        next.updated_at = now_ms;
    }
    (next, corrections)
}

/// Builds normalized-alias → (canonical, term_id). Aliases AND the
/// canonical term map to the canonical spelling.
fn build_lookup(terms: &[GlossaryTerm]) -> std::collections::HashMap<String, (String, String)> {
    let mut map = std::collections::HashMap::new();
    for t in terms {
        let entry = (t.term.clone(), t.id.clone());
        map.insert(normalize(&t.term), entry.clone());
        for alias in &t.aliases {
            // Don't let an alias override an exact canonical match from
            // another term (canonical wins).
            map.entry(normalize(alias)).or_insert_with(|| entry.clone());
        }
    }
    map
}

/// Lowercase, trim — for case-insensitive comparison.
fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Split a transcript token into (leading punctuation, core, trailing
/// punctuation) so "(kerigma)," → ("(", "kerigma", "),").
fn split_affixes(s: &str) -> (&str, &str, &str) {
    let is_word_char = |c: char| c.is_alphanumeric() || c == '\'' || c == '-';
    let start = s.find(is_word_char).unwrap_or(s.len());
    let end = s
        .rfind(is_word_char)
        .map(|i| i + s[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(start);
    (&s[..start], &s[start..end], &s[end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, GlossaryTerm, Style, Word};

    fn project_with(words: Vec<Word>, glossary: Vec<GlossaryTerm>) -> Project {
        Project {
            id: "p".into(),
            name: "t".into(),
            video_path: "/x".into(),
            video_content_hash: "h".into(),
            video_duration_ms: 1000,
            video_width: 0,
            video_height: 0,
            video_fps: 0.0,
            audio_wav_path: None,
            language: "no".into(),
            default_style: Style::broadcast_news(),
            context_description: None,
            captions: vec![Caption {
                id: "c1".into(),
                start_ms: 0,
                end_ms: 1000,
                words,
                speaker_id: None,
                style_id: None,
                notes: None,
                ai_generated: true,
                last_edited_at: 0,
                track_id: None,
            }],
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

    fn term(id: &str, canonical: &str, aliases: &[&str]) -> GlossaryTerm {
        GlossaryTerm {
            id: id.into(),
            term: canonical.into(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            definition: None,
            pronunciation_hint: None,
        }
    }

    #[test]
    fn corrects_alias_to_canonical() {
        let p = project_with(
            vec![Word::new("kerigma", 0, 500, 40.0)],
            vec![term("g1", "kerygma", &["kerigma", "karisma"])],
        );
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "kerygma");
        assert!(out.captions[0].words[0].edited);
        assert_eq!(corr.len(), 1);
        assert_eq!(corr[0].from, "kerigma");
        assert_eq!(corr[0].to, "kerygma");
        assert_eq!(corr[0].term_id, "g1");
    }

    #[test]
    fn preserves_trailing_punctuation() {
        let p = project_with(
            vec![Word::new("kerigma,", 0, 500, 40.0)],
            vec![term("g1", "kerygma", &["kerigma"])],
        );
        let (out, _) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "kerygma,");
    }

    #[test]
    fn preserves_surrounding_parens() {
        let p = project_with(
            vec![Word::new("(kerigma).", 0, 500, 40.0)],
            vec![term("g1", "kerygma", &["kerigma"])],
        );
        let (out, _) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "(kerygma).");
    }

    #[test]
    fn case_insensitive_alias_match() {
        let p = project_with(
            vec![Word::new("KERIGMA", 0, 500, 40.0)],
            vec![term("g1", "Kerygma", &["kerigma"])],
        );
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "Kerygma");
        assert_eq!(corr.len(), 1);
    }

    #[test]
    fn already_correct_word_is_untouched() {
        let p = project_with(
            vec![Word::new("kerygma", 0, 500, 90.0)],
            vec![term("g1", "kerygma", &["kerigma"])],
        );
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "kerygma");
        assert!(!out.captions[0].words[0].edited, "no spurious edit flag");
        assert!(corr.is_empty());
    }

    #[test]
    fn respects_user_edited_words() {
        let mut w = Word::new("kerigma", 0, 500, 40.0);
        w.edited = true; // user already touched it
        let p = project_with(vec![w], vec![term("g1", "kerygma", &["kerigma"])]);
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(
            out.captions[0].words[0].text, "kerigma",
            "don't override user edits"
        );
        assert!(corr.is_empty());
    }

    #[test]
    fn respects_locked_words() {
        let mut w = Word::new("kerigma", 0, 500, 40.0);
        w.locked = true;
        let p = project_with(vec![w], vec![term("g1", "kerygma", &["kerigma"])]);
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "kerigma");
        assert!(corr.is_empty());
    }

    #[test]
    fn empty_glossary_is_noop() {
        let p = project_with(vec![Word::new("anything", 0, 500, 40.0)], vec![]);
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out, p);
        assert!(corr.is_empty());
    }

    #[test]
    fn multiple_corrections_across_words() {
        let p = project_with(
            vec![
                Word::new("kerigma", 0, 300, 40.0),
                Word::new("and", 300, 400, 95.0),
                Word::new("soteorologi", 400, 900, 35.0),
            ],
            vec![
                term("g1", "kerygma", &["kerigma"]),
                term("g2", "soteriologi", &["soteorologi"]),
            ],
        );
        let (out, corr) = apply_glossary(&p, 100);
        assert_eq!(out.captions[0].words[0].text, "kerygma");
        assert_eq!(out.captions[0].words[1].text, "and"); // untouched
        assert_eq!(out.captions[0].words[2].text, "soteriologi");
        assert_eq!(corr.len(), 2);
    }

    #[test]
    fn split_affixes_works() {
        assert_eq!(split_affixes("word"), ("", "word", ""));
        assert_eq!(split_affixes("word,"), ("", "word", ","));
        assert_eq!(split_affixes("(word)."), ("(", "word", ")."));
        assert_eq!(split_affixes("\"word\""), ("\"", "word", "\""));
        assert_eq!(split_affixes("don't"), ("", "don't", ""));
        assert_eq!(split_affixes("..."), ("...", "", ""));
    }
}
