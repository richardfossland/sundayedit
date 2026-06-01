//! Caption quality + re-flow — readability operations (Phase 7 depth).
//!
//! Professional captioning has hard readability limits that ASR output
//! routinely violates: too many characters on screen per second (CPS),
//! lines too long to read, or too many lines stacked at once. This module
//! is the pure analysis + repair logic for those constraints.
//!
//! Four pure pieces, all `Project`/`Caption` in → value or new state out:
//!
//!   1. `cps`            — characters-per-second for a caption (the single
//!      most-cited subtitle readability metric; broadcast targets ~17 CPS,
//!      Netflix caps at 17 for adults / 20 absolute).
//!   2. `wrap_lines`     — greedy line-break a caption's text into at most
//!      `max_lines` lines no wider than `max_chars_per_line`. Mirrors the
//!      box-wrapping libass does, so the editor preview and burn-in agree.
//!   3. `analyze`        — flag every caption that breaks a `ReflowConfig`
//!      limit (CPS / line length / line count / min-duration), so the UI can
//!      surface them the same way confidence highlighting surfaces words.
//!   4. `snap_caption_to_words` / `retime_caption_even` — pure timing repair:
//!      tighten a caption's box to its words, or redistribute word timings
//!      evenly across the caption when ASR word-timing is missing/garbage.
//!
//! Everything here is deliberately Project-free where it can be (string and
//! number math) so it's trivially unit-tested without building a fixture.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::model::{Caption, Project};

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ReflowConfig.ts")]
pub struct ReflowConfig {
    /// Max readable characters-per-second. Broadcast/Netflix adult default 17.
    pub max_cps: f32,
    /// Max characters on a single line before it must wrap. BBC uses 37–40.
    pub max_chars_per_line: usize,
    /// Max lines visible at once (2 is the broadcast standard).
    pub max_lines: usize,
    /// Minimum on-screen time in ms. Below this a caption flashes (Netflix 833ms ≈ 5/6 s).
    #[ts(type = "number")]
    pub min_duration_ms: i64,
}

impl Default for ReflowConfig {
    fn default() -> Self {
        // Conservative broadcast defaults — see module docs for sources.
        Self {
            max_cps: 17.0,
            max_chars_per_line: 37,
            max_lines: 2,
            min_duration_ms: 833,
        }
    }
}

// ── CPS ─────────────────────────────────────────────────────────────────────

/// Characters-per-second for a caption: visible characters (the rendered
/// text length, counting spaces, by Unicode scalar) over its on-screen
/// duration in seconds. A zero/negative-duration caption reports `f32::INFINITY`
/// so it always trips the limit rather than dividing by zero.
pub fn cps(caption: &Caption) -> f32 {
    let chars = caption.text().chars().count() as f32;
    let dur_s = (caption.end_ms - caption.start_ms) as f32 / 1000.0;
    if dur_s <= 0.0 {
        return f32::INFINITY;
    }
    chars / dur_s
}

// ── Line wrapping ─────────────────────────────────────────────────────────────

/// Greedy word-wrap `text` into lines no wider than `max_chars` (by char
/// count, including the joining spaces). A single word longer than `max_chars`
/// gets its own line rather than being split mid-word — splitting words is
/// never acceptable in subtitles.
///
/// Returns the lines in order. Whitespace is collapsed to single spaces.
pub fn wrap_lines(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        // +1 for the space we'd insert.
        if current.chars().count() + 1 + word.chars().count() <= max_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Balance a two-line wrap so the lines are closer in length ("top-heavy"
/// reads better than a long line over a stub). Only rebalances when the wrap
/// produced exactly two lines and rebalancing keeps both within `max_chars`.
/// Returns the (possibly rebalanced) lines.
pub fn wrap_balanced(text: &str, max_chars: usize) -> Vec<String> {
    let greedy = wrap_lines(text, max_chars);
    if greedy.len() != 2 {
        return greedy;
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 2 {
        return greedy;
    }
    // Try every split point; pick the one that minimises the line-length
    // difference while keeping both lines within max_chars. Prefer a
    // top-heavy break (first line >= second) on ties — the subtitle norm.
    let mut best: Option<(usize, Vec<String>)> = None;
    for split in 1..words.len() {
        let top = words[..split].join(" ");
        let bottom = words[split..].join(" ");
        let tlen = top.chars().count();
        let blen = bottom.chars().count();
        if tlen > max_chars || blen > max_chars {
            continue;
        }
        // Cost: length difference; tie-break toward top-heavy (smaller when top >= bottom).
        let diff = tlen.abs_diff(blen);
        let cost = diff * 2 + usize::from(tlen < blen);
        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
            best = Some((cost, vec![top, bottom]));
        }
    }
    best.map(|(_, l)| l).unwrap_or(greedy)
}

// ── Analysis ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ReflowIssue.ts")]
pub struct ReflowIssue {
    pub caption_id: String,
    /// "cps" | "line_length" | "line_count" | "min_duration"
    pub kind: String,
    /// The offending measured value (CPS, longest line length, line count, or duration ms).
    pub value: f32,
    /// The limit it broke.
    pub limit: f32,
    /// Human-readable one-liner for the UI.
    pub message: String,
}

/// Flag every caption that breaks a readability limit. One caption can yield
/// several issues (e.g. both too-fast and too-long). Empty result = clean.
pub fn analyze(project: &Project, cfg: &ReflowConfig) -> Vec<ReflowIssue> {
    let mut issues = Vec::new();
    for c in &project.captions {
        if c.words.is_empty() {
            continue;
        }
        let id = c.id.clone();

        let c_cps = cps(c);
        if c_cps > cfg.max_cps {
            issues.push(ReflowIssue {
                caption_id: id.clone(),
                kind: "cps".into(),
                value: c_cps,
                limit: cfg.max_cps,
                message: format!("{c_cps:.1} CPS exceeds {:.0} — reads too fast", cfg.max_cps),
            });
        }

        let lines = wrap_lines(&c.text(), cfg.max_chars_per_line);
        let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        if longest > cfg.max_chars_per_line {
            issues.push(ReflowIssue {
                caption_id: id.clone(),
                kind: "line_length".into(),
                value: longest as f32,
                limit: cfg.max_chars_per_line as f32,
                message: format!(
                    "line of {longest} chars exceeds {} — a single word is too long to wrap",
                    cfg.max_chars_per_line
                ),
            });
        }
        if lines.len() > cfg.max_lines {
            issues.push(ReflowIssue {
                caption_id: id.clone(),
                kind: "line_count".into(),
                value: lines.len() as f32,
                limit: cfg.max_lines as f32,
                message: format!(
                    "{} lines exceeds {} — split this caption",
                    lines.len(),
                    cfg.max_lines
                ),
            });
        }

        let dur = c.end_ms - c.start_ms;
        if dur < cfg.min_duration_ms {
            issues.push(ReflowIssue {
                caption_id: id,
                kind: "min_duration".into(),
                value: dur as f32,
                limit: cfg.min_duration_ms as f32,
                message: format!("{dur}ms on screen is below {}ms — flashes", cfg.min_duration_ms),
            });
        }
    }
    issues
}

// ── Timing repair ───────────────────────────────────────────────────────────

/// Tighten (snap) a caption's box to its words: `start_ms` = first word start,
/// `end_ms` = last word end. No-op if the caption has no words. Keeps the
/// project valid because snapping inward never creates an overlap.
pub fn snap_caption_to_words(
    project: &Project,
    caption_id: &str,
    now_ms: i64,
) -> AppResult<Project> {
    let mut next = project.clone();
    let cap = next
        .captions
        .iter_mut()
        .find(|c| c.id == caption_id)
        .ok_or_else(|| AppError::NotFound {
            entity: "caption",
            id: caption_id.to_string(),
        })?;
    if cap.words.is_empty() {
        return Ok(project.clone());
    }
    cap.start_ms = cap.words.first().unwrap().start_ms;
    cap.end_ms = cap.words.last().unwrap().end_ms;
    cap.last_edited_at = now_ms;
    next.updated_at = now_ms;
    next.validate().map_err(AppError::Invariant)?;
    Ok(next)
}

/// Redistribute a caption's word timings evenly across its `[start_ms, end_ms]`
/// box. Used when ASR returned a caption-level time range but garbage (or no)
/// per-word timing — even spacing is a defensible, monotonic fallback that the
/// confidence-highlighting word stepping needs in order to work at all.
///
/// Word order and text are preserved; only `start_ms`/`end_ms` per word change.
pub fn retime_caption_even(
    project: &Project,
    caption_id: &str,
    now_ms: i64,
) -> AppResult<Project> {
    let mut next = project.clone();
    let cap = next
        .captions
        .iter_mut()
        .find(|c| c.id == caption_id)
        .ok_or_else(|| AppError::NotFound {
            entity: "caption",
            id: caption_id.to_string(),
        })?;
    let n = cap.words.len();
    if n == 0 {
        return Ok(project.clone());
    }
    let span = cap.end_ms - cap.start_ms;
    if span < n as i64 {
        return Err(AppError::Validation(format!(
            "caption span {span}ms too short to give {n} words ≥1ms each"
        )));
    }
    // Integer-fair partition: distribute the remainder across the first words
    // so the boundaries stay monotonic and the last word ends exactly at end_ms.
    let base = span / n as i64;
    let rem = span % n as i64;
    let mut t = cap.start_ms;
    for (i, w) in cap.words.iter_mut().enumerate() {
        let slot = base + i64::from((i as i64) < rem);
        w.start_ms = t;
        w.end_ms = t + slot;
        t += slot;
    }
    cap.last_edited_at = now_ms;
    next.updated_at = now_ms;
    next.validate().map_err(AppError::Invariant)?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Project, Style, Word};

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
        }
    }

    fn w(text: &str, s: i64, e: i64) -> Word {
        Word::new(text, s, e, 90.0)
    }

    // ── cps ──────────────────────────────────────────────────────────────────
    #[test]
    fn cps_counts_chars_over_seconds() {
        // "Hello world" = 11 chars, over 1s = 11 CPS.
        let c = caption("c", 0, 1000, vec![w("Hello", 0, 500), w("world", 500, 1000)]);
        assert!((cps(&c) - 11.0).abs() < 0.001);
    }

    #[test]
    fn cps_infinite_on_zero_duration() {
        let c = caption("c", 1000, 1000, vec![w("x", 1000, 1000)]);
        assert!(cps(&c).is_infinite());
    }

    // ── wrap_lines ─────────────────────────────────────────────────────────────
    #[test]
    fn wrap_greedy_breaks_at_limit() {
        // "the cat sat" with max 7 → "the cat" (7), "sat".
        let lines = wrap_lines("the cat sat", 7);
        assert_eq!(lines, vec!["the cat", "sat"]);
    }

    #[test]
    fn wrap_keeps_overlong_word_whole() {
        let lines = wrap_lines("a supercalifragilistic b", 5);
        // The long word gets its own line rather than being split.
        assert_eq!(lines, vec!["a", "supercalifragilistic", "b"]);
    }

    #[test]
    fn wrap_collapses_whitespace() {
        let lines = wrap_lines("  one   two  ", 100);
        assert_eq!(lines, vec!["one two"]);
    }

    #[test]
    fn wrap_unicode_counts_scalars_not_bytes() {
        // "café" is 4 chars but 5 bytes; with max 4 it must fit on one line.
        let lines = wrap_lines("café té", 4);
        assert_eq!(lines, vec!["café", "té"]);
    }

    // ── wrap_balanced ──────────────────────────────────────────────────────────
    #[test]
    fn balanced_evens_out_a_lopsided_break() {
        // Greedy would pack the first line full and strand a stub; balanced
        // makes the two lines closer in length.
        let text = "one two three four five";
        let greedy = wrap_lines(text, 14);
        let balanced = wrap_balanced(text, 14);
        assert_eq!(greedy.len(), 2);
        assert_eq!(balanced.len(), 2);
        let g_diff = greedy[0].len().abs_diff(greedy[1].len());
        let b_diff = balanced[0].len().abs_diff(balanced[1].len());
        assert!(b_diff <= g_diff);
    }

    #[test]
    fn balanced_prefers_top_heavy_on_ties() {
        // "ab cd ef" with width 8 → balanced two-line, top line not shorter.
        let lines = wrap_balanced("aaa bbb ccc", 7);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].chars().count() >= lines[1].chars().count());
    }

    #[test]
    fn balanced_passthrough_when_one_line() {
        assert_eq!(wrap_balanced("short", 80), vec!["short"]);
    }

    // ── analyze ────────────────────────────────────────────────────────────────
    #[test]
    fn analyze_flags_fast_caption() {
        // 30 chars in 1s = 30 CPS, way over 17.
        let c = caption(
            "fast",
            0,
            1000,
            vec![w("abcdefghij", 0, 500), w("klmnopqrst", 500, 1000)],
        );
        let issues = analyze(&proj(vec![c]), &ReflowConfig::default());
        assert!(issues.iter().any(|i| i.kind == "cps"));
    }

    #[test]
    fn analyze_flags_too_long_unbreakable_line() {
        let long = "x".repeat(50);
        let c = caption("long", 0, 10_000, vec![w(&long, 0, 5000)]);
        let issues = analyze(&proj(vec![c]), &ReflowConfig::default());
        let line = issues.iter().find(|i| i.kind == "line_length").unwrap();
        assert_eq!(line.value, 50.0);
    }

    #[test]
    fn analyze_flags_too_many_lines() {
        // Five short words, max 3 chars/line → 5 lines, over max_lines 2.
        let c = caption(
            "many",
            0,
            10_000,
            vec![
                w("aaa", 0, 2000),
                w("bbb", 2000, 4000),
                w("ccc", 4000, 6000),
                w("ddd", 6000, 8000),
                w("eee", 8000, 10_000),
            ],
        );
        let cfg = ReflowConfig {
            max_chars_per_line: 3,
            ..ReflowConfig::default()
        };
        let issues = analyze(&proj(vec![c]), &cfg);
        assert!(issues.iter().any(|i| i.kind == "line_count" && i.value == 5.0));
    }

    #[test]
    fn analyze_flags_flash_caption() {
        let c = caption("flash", 0, 300, vec![w("hi", 0, 300)]);
        let issues = analyze(&proj(vec![c]), &ReflowConfig::default());
        let d = issues.iter().find(|i| i.kind == "min_duration").unwrap();
        assert_eq!(d.value, 300.0);
    }

    #[test]
    fn analyze_clean_caption_has_no_issues() {
        // "Hello there" 11 chars over 2s = 5.5 CPS, one short line, 2s on screen.
        let c = caption(
            "ok",
            0,
            2000,
            vec![w("Hello", 0, 1000), w("there", 1000, 2000)],
        );
        assert!(analyze(&proj(vec![c]), &ReflowConfig::default()).is_empty());
    }

    #[test]
    fn analyze_skips_empty_captions() {
        let c = caption("empty", 0, 100, vec![]);
        assert!(analyze(&proj(vec![c]), &ReflowConfig::default()).is_empty());
    }

    // ── snap_caption_to_words ───────────────────────────────────────────────────
    #[test]
    fn snap_tightens_box_to_words() {
        // Caption box 0..5000 but words only span 1000..3000.
        let c = caption(
            "c",
            0,
            5000,
            vec![w("a", 1000, 2000), w("b", 2000, 3000)],
        );
        let out = snap_caption_to_words(&proj(vec![c]), "c", 9).unwrap();
        assert_eq!(out.captions[0].start_ms, 1000);
        assert_eq!(out.captions[0].end_ms, 3000);
        out.validate().unwrap();
    }

    #[test]
    fn snap_missing_caption_errors() {
        let c = caption("c", 0, 1000, vec![w("a", 0, 1000)]);
        let err = snap_caption_to_words(&proj(vec![c]), "nope", 9).unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    // ── retime_caption_even ─────────────────────────────────────────────────────
    #[test]
    fn retime_even_distributes_words() {
        // 3 words across 0..900 → 300ms each.
        let c = caption(
            "c",
            0,
            900,
            vec![w("a", 0, 10), w("b", 10, 20), w("c", 20, 30)],
        );
        let out = retime_caption_even(&proj(vec![c]), "c", 9).unwrap();
        let ws = &out.captions[0].words;
        assert_eq!((ws[0].start_ms, ws[0].end_ms), (0, 300));
        assert_eq!((ws[1].start_ms, ws[1].end_ms), (300, 600));
        assert_eq!((ws[2].start_ms, ws[2].end_ms), (600, 900));
        out.validate().unwrap();
    }

    #[test]
    fn retime_even_remainder_goes_to_first_words_and_ends_exactly() {
        // 4 words across 0..1000 → 1000/4 = 250 base, 0 remainder; clean.
        // 3 words across 0..1000 → base 333, rem 1 → first word gets +1.
        let c = caption(
            "c",
            0,
            1000,
            vec![w("a", 0, 1), w("b", 1, 2), w("c", 2, 3)],
        );
        let out = retime_caption_even(&proj(vec![c]), "c", 9).unwrap();
        let ws = &out.captions[0].words;
        assert_eq!(ws[0].start_ms, 0);
        assert_eq!(ws[0].end_ms, 334); // 333 + 1 remainder
        assert_eq!(ws[1].start_ms, 334);
        assert_eq!(ws.last().unwrap().end_ms, 1000); // exact landing
        out.validate().unwrap();
    }

    #[test]
    fn retime_even_too_short_span_errors() {
        // 3 words but only 2ms of span — can't give each ≥1ms.
        let c = caption(
            "c",
            0,
            2,
            vec![w("a", 0, 1), w("b", 1, 2), w("c", 1, 2)],
        );
        let err = retime_caption_even(&proj(vec![c]), "c", 9).unwrap_err();
        assert_eq!(err.code(), "validation");
    }
}
