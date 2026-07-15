//! Find & replace + bulk operations — Phase 7.3.
//!
//! Boring but essential. Searches operate at the WORD level: each word's
//! text is matched independently. This handles the overwhelmingly common
//! cases (fix a recurring mis-spelling, swap a name) cleanly given our
//! word-based model. Phrase-across-word matching is a v2 concern and is
//! noted where it matters.
//!
//! All functions are pure: Project in, (Project or matches) out.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::Project;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/FindOptions.ts")]
pub struct FindOptions {
    pub query: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// Treat `query` as a regular expression.
    pub regex: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/FindMatch.ts")]
pub struct FindMatch {
    pub caption_id: String,
    pub word_index: usize,
    /// Char offsets of the match within the word's text.
    pub start: usize,
    pub end: usize,
    /// The matched substring (for preview).
    pub matched: String,
}

/// Compiled matcher — built once, reused across every word.
enum Matcher {
    Plain {
        needle: String,
        case_sensitive: bool,
        whole_word: bool,
    },
    Regex(regex::Regex),
}

impl Matcher {
    fn build(opts: &FindOptions) -> AppResult<Self> {
        if opts.query.is_empty() {
            return Err(AppError::Validation("search query is empty".into()));
        }
        if opts.regex {
            let pattern = if opts.case_sensitive {
                opts.query.clone()
            } else {
                format!("(?i){}", opts.query)
            };
            let re = regex::Regex::new(&pattern)
                .map_err(|e| AppError::Validation(format!("invalid regex: {e}")))?;
            Ok(Matcher::Regex(re))
        } else {
            Ok(Matcher::Plain {
                needle: opts.query.clone(),
                case_sensitive: opts.case_sensitive,
                whole_word: opts.whole_word,
            })
        }
    }

    /// All (start, end, matched) within `text`.
    fn find_in(&self, text: &str) -> Vec<(usize, usize, String)> {
        match self {
            Matcher::Regex(re) => re
                .find_iter(text)
                // Drop zero-width matches (`\b`, `a*`, lookarounds). At the
                // word level they carry no text to find or replace: they'd
                // surface as empty `matched` previews and, in `replace_all`,
                // insert `replacement` at every boundary while inflating the
                // count — neither is what "find & replace" means.
                .filter(|m| m.end() > m.start())
                .map(|m| (m.start(), m.end(), m.as_str().to_string()))
                .collect(),
            Matcher::Plain {
                needle,
                case_sensitive,
                whole_word,
            } => {
                let need = if *case_sensitive {
                    needle.clone()
                } else {
                    needle.to_lowercase()
                };
                if need.is_empty() {
                    return vec![];
                }
                let mut out = Vec::new();
                if *case_sensitive {
                    // Offsets into `text` are valid directly.
                    let mut from = 0;
                    while let Some(pos) = text[from..].find(&need) {
                        let start = from + pos;
                        let end = start + need.len();
                        if !*whole_word || is_whole_word(text, start, end) {
                            out.push((start, end, text[start..end].to_string()));
                        }
                        from = end.max(start + 1);
                    }
                } else {
                    // `str::to_lowercase` is NOT length-preserving (e.g. Turkish
                    // 'İ'), so byte offsets into the lowercased haystack do not map
                    // back to `text`. Instead, scan each char boundary of the
                    // ORIGINAL `text` and compare a lowercased slice anchored there.
                    let indices: Vec<usize> = text
                        .char_indices()
                        .map(|(i, _)| i)
                        .chain(std::iter::once(text.len()))
                        .collect();
                    let mut i = 0;
                    while i + 1 < indices.len() {
                        let start = indices[i];
                        // Find the char boundary `j > i` whose lowercased slice
                        // `text[start..indices[j]]` equals `need`.
                        let mut matched_j = None;
                        for (offset, &end) in indices[i + 1..].iter().enumerate() {
                            let slice_lower = text[start..end].to_lowercase();
                            if slice_lower == need {
                                matched_j = Some(i + 1 + offset);
                                break;
                            }
                            if !need.starts_with(&slice_lower) {
                                // Lowercased prefix can no longer extend to `need`.
                                break;
                            }
                        }
                        if let Some(j) = matched_j {
                            let end = indices[j];
                            if !*whole_word || is_whole_word(text, start, end) {
                                out.push((start, end, text[start..end].to_string()));
                            }
                            // Non-overlapping: resume at the match end (≥ one char).
                            i = j.max(i + 1);
                        } else {
                            i += 1;
                        }
                    }
                }
                out
            }
        }
    }
}

fn is_whole_word(text: &str, start: usize, end: usize) -> bool {
    let before_ok = start == 0
        || !text[..start]
            .chars()
            .last()
            .map(is_word_char)
            .unwrap_or(false);
    let after_ok = end >= text.len()
        || !text[end..]
            .chars()
            .next()
            .map(is_word_char)
            .unwrap_or(false);
    before_ok && after_ok
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Find every match across the project.
pub fn find_all(project: &Project, opts: &FindOptions) -> AppResult<Vec<FindMatch>> {
    let matcher = Matcher::build(opts)?;
    let mut matches = Vec::new();
    for cap in &project.captions {
        for (wi, word) in cap.words.iter().enumerate() {
            for (start, end, matched) in matcher.find_in(&word.text) {
                matches.push(FindMatch {
                    caption_id: cap.id.clone(),
                    word_index: wi,
                    start,
                    end,
                    matched,
                });
            }
        }
    }
    Ok(matches)
}

/// Replace every match with `replacement`. Returns the new project and the
/// number of replacements made. Words whose text changes are marked edited.
pub fn replace_all(
    project: &Project,
    opts: &FindOptions,
    replacement: &str,
    now_ms: i64,
) -> AppResult<(Project, usize)> {
    let matcher = Matcher::build(opts)?;
    let mut next = project.clone();
    let mut count = 0;

    for cap in next.captions.iter_mut() {
        let mut changed = false;
        for word in cap.words.iter_mut() {
            let hits = matcher.find_in(&word.text);
            if hits.is_empty() {
                continue;
            }
            // Rebuild text applying replacements right-to-left so offsets stay valid.
            let mut new_text = word.text.clone();
            for (start, end, _) in hits.iter().rev() {
                // A match already equal to `replacement` is a no-op; skip it so
                // the reported count reflects real changes (replacing a term
                // with itself must not claim "N replacements made"). Offsets are
                // against the unmodified `word.text`, which we never mutate.
                if word.text.get(*start..*end) == Some(replacement) {
                    continue;
                }
                new_text.replace_range(*start..*end, replacement);
                count += 1;
            }
            // A replacement that empties the word: keep a single space-free
            // marker would be wrong; instead drop later. For v1 we keep the
            // (possibly empty) word and let the user clean up, but trim.
            if new_text != word.text {
                word.text = new_text;
                word.edited = true;
                changed = true;
            }
        }
        if changed {
            cap.last_edited_at = now_ms;
            // Drop any words that became empty after replacement.
            cap.words.retain(|w| !w.text.trim().is_empty());
        }
    }
    // Drop captions that lost all their words.
    next.captions.retain(|c| !c.words.is_empty());
    if count > 0 {
        next.updated_at = now_ms;
    }
    Ok((next, count))
}

// ── Bulk operations ───────────────────────────────────────────────────────────

/// Delete a set of captions by id.
pub fn bulk_delete(project: &Project, caption_ids: &[String], now_ms: i64) -> Project {
    let set: std::collections::HashSet<&str> = caption_ids.iter().map(|s| s.as_str()).collect();
    let mut next = project.clone();
    let before = next.captions.len();
    next.captions.retain(|c| !set.contains(c.id.as_str()));
    if next.captions.len() != before {
        next.updated_at = now_ms;
    }
    next
}

/// Assign a speaker to a set of captions.
pub fn bulk_set_speaker(
    project: &Project,
    caption_ids: &[String],
    speaker_id: Option<String>,
    now_ms: i64,
) -> Project {
    let set: std::collections::HashSet<&str> = caption_ids.iter().map(|s| s.as_str()).collect();
    let mut next = project.clone();
    for c in next.captions.iter_mut() {
        if set.contains(c.id.as_str()) {
            c.speaker_id = speaker_id.clone();
            c.last_edited_at = now_ms;
        }
    }
    next.updated_at = now_ms;
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Project, Style, Word};

    fn proj(captions: Vec<(&str, Vec<&str>)>) -> Project {
        let caps = captions
            .into_iter()
            .enumerate()
            .map(|(ci, (id, words))| {
                let ws = words
                    .into_iter()
                    .enumerate()
                    .map(|(wi, t)| Word::new(t, (wi as i64) * 500, (wi as i64) * 500 + 400, 90.0))
                    .collect();
                Caption {
                    id: id.to_string(),
                    start_ms: ci as i64 * 5000,
                    end_ms: ci as i64 * 5000 + 2000,
                    words: ws,
                    speaker_id: None,
                    style_id: None,
                    notes: None,
                    ai_generated: true,
                    last_edited_at: 0,
                    track_id: None,
                }
            })
            .collect();
        Project {
            id: "p".into(),
            name: "t".into(),
            video_path: "/x".into(),
            video_content_hash: "h".into(),
            video_duration_ms: 60000,
            video_width: 1920,
            video_height: 1080,
            video_fps: 30.0,
            audio_wav_path: None,
            language: "en".into(),
            default_style: Style::broadcast_news(),
            context_description: None,
            captions: caps,
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

    fn opts(q: &str) -> FindOptions {
        FindOptions {
            query: q.into(),
            case_sensitive: false,
            whole_word: false,
            regex: false,
        }
    }

    #[test]
    fn find_plain_case_insensitive() {
        let p = proj(vec![("c1", vec!["Hello", "WORLD", "hello"])]);
        let m = find_all(&p, &opts("hello")).unwrap();
        assert_eq!(m.len(), 2); // "Hello" + "hello"
    }

    #[test]
    fn find_case_sensitive() {
        let p = proj(vec![("c1", vec!["Hello", "hello"])]);
        let o = FindOptions {
            query: "hello".into(),
            case_sensitive: true,
            whole_word: false,
            regex: false,
        };
        let m = find_all(&p, &o).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].matched, "hello");
    }

    #[test]
    fn find_whole_word_only() {
        let p = proj(vec![("c1", vec!["cat", "category", "scatter"])]);
        let o = FindOptions {
            query: "cat".into(),
            case_sensitive: false,
            whole_word: true,
            regex: false,
        };
        let m = find_all(&p, &o).unwrap();
        assert_eq!(m.len(), 1); // only "cat", not "category"/"scatter"
        assert_eq!(m[0].word_index, 0);
    }

    #[test]
    fn find_regex() {
        let p = proj(vec![("c1", vec!["2024", "abc", "1999"])]);
        let o = FindOptions {
            query: r"\d{4}".into(),
            case_sensitive: false,
            whole_word: false,
            regex: false,
        };
        let o = FindOptions { regex: true, ..o };
        let m = find_all(&p, &o).unwrap();
        assert_eq!(m.len(), 2); // 2024, 1999
    }

    #[test]
    fn invalid_regex_errors() {
        let p = proj(vec![("c1", vec!["x"])]);
        let o = FindOptions {
            query: "(".into(),
            case_sensitive: false,
            whole_word: false,
            regex: true,
        };
        assert_eq!(find_all(&p, &o).unwrap_err().code(), "validation");
    }

    #[test]
    fn empty_query_errors() {
        let p = proj(vec![("c1", vec!["x"])]);
        assert_eq!(find_all(&p, &opts("")).unwrap_err().code(), "validation");
    }

    #[test]
    fn replace_all_counts_and_edits() {
        let p = proj(vec![("c1", vec!["color", "colored", "colorful"])]);
        let (out, count) = replace_all(&p, &opts("color"), "colour", 100).unwrap();
        assert_eq!(count, 3);
        assert_eq!(out.captions[0].words[0].text, "colour");
        assert_eq!(out.captions[0].words[1].text, "coloured");
        assert_eq!(out.captions[0].words[2].text, "colourful");
        assert!(out.captions[0].words.iter().all(|w| w.edited));
    }

    #[test]
    fn replace_to_empty_drops_word() {
        // Replacing "um" with "" should remove the filler word entirely.
        let p = proj(vec![("c1", vec!["So", "um", "yes"])]);
        let o = FindOptions {
            query: "um".into(),
            case_sensitive: false,
            whole_word: true,
            regex: false,
        };
        let (out, count) = replace_all(&p, &o, "", 100).unwrap();
        assert_eq!(count, 1);
        assert_eq!(out.captions[0].words.len(), 2);
        assert_eq!(out.captions[0].text(), "So yes");
    }

    #[test]
    fn replace_emptying_all_words_drops_caption() {
        let p = proj(vec![("c1", vec!["um"]), ("c2", vec!["real"])]);
        let o = FindOptions {
            query: "um".into(),
            case_sensitive: false,
            whole_word: true,
            regex: false,
        };
        let (out, _) = replace_all(&p, &o, "", 100).unwrap();
        assert_eq!(out.captions.len(), 1);
        assert_eq!(out.captions[0].id, "c2");
    }

    #[test]
    fn case_insensitive_match_with_length_changing_lowercase() {
        // Turkish dotted capital I (U+0130, 2 bytes) lowercases to "i" + combining
        // dot (3 bytes). A case-insensitive search must not slice the ORIGINAL text
        // using offsets computed against the lowercased haystack, or it panics on a
        // char boundary / out-of-bounds index.
        let p = proj(vec![("c1", vec!["\u{0130}um"])]); // "İum"
        let m = find_all(&p, &opts("um")).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].matched, "um");

        let (out, count) = replace_all(&p, &opts("um"), "X", 100).unwrap();
        assert_eq!(count, 1);
        assert_eq!(out.captions[0].words[0].text, "\u{0130}X");
    }

    #[test]
    fn find_skips_zero_width_regex() {
        // `\b` matches at every word boundary but consumes nothing. Such matches
        // are meaningless for find/replace and must not appear.
        let p = proj(vec![("c1", vec!["hello", "world"])]);
        let o = FindOptions {
            query: r"\b".into(),
            case_sensitive: false,
            whole_word: false,
            regex: true,
        };
        let m = find_all(&p, &o).unwrap();
        assert!(
            m.is_empty(),
            "zero-width matches must be dropped, got {m:?}"
        );
    }

    #[test]
    fn replace_zero_width_regex_is_noop() {
        // A zero-width pattern must neither insert text nor inflate the count.
        let p = proj(vec![("c1", vec!["hello"])]);
        let o = FindOptions {
            query: r"\b".into(),
            case_sensitive: false,
            whole_word: false,
            regex: true,
        };
        let (out, count) = replace_all(&p, &o, "X", 100).unwrap();
        assert_eq!(count, 0);
        assert_eq!(out.captions[0].words[0].text, "hello");
        assert!(!out.captions[0].words[0].edited);
    }

    #[test]
    fn replace_with_self_is_not_counted() {
        // Replacing a term with itself changes nothing — the count must be 0 and
        // the word must not be marked edited (no false "unsaved changes").
        let p = proj(vec![("c1", vec!["color", "colorful"])]);
        let o = FindOptions {
            query: "color".into(),
            case_sensitive: true,
            whole_word: false,
            regex: false,
        };
        let (out, count) = replace_all(&p, &o, "color", 100).unwrap();
        assert_eq!(count, 0);
        assert_eq!(out.captions[0].words[0].text, "color");
        assert_eq!(out.captions[0].words[1].text, "colorful");
        assert!(out.captions[0].words.iter().all(|w| !w.edited));
        assert_eq!(out.updated_at, 0); // untouched
    }

    #[test]
    fn bulk_delete_removes_captions() {
        let p = proj(vec![
            ("c1", vec!["a"]),
            ("c2", vec!["b"]),
            ("c3", vec!["c"]),
        ]);
        let out = bulk_delete(&p, &["c1".into(), "c3".into()], 100);
        assert_eq!(out.captions.len(), 1);
        assert_eq!(out.captions[0].id, "c2");
    }

    #[test]
    fn bulk_set_speaker_applies() {
        let p = proj(vec![("c1", vec!["a"]), ("c2", vec!["b"])]);
        let out = bulk_set_speaker(&p, &["c1".into()], Some("s1".into()), 100);
        assert_eq!(out.captions[0].speaker_id.as_deref(), Some("s1"));
        assert_eq!(out.captions[1].speaker_id, None);
    }
}
