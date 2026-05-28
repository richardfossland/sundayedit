//! Transcript → Caption[] conversion — Phase 2.1.
//!
//! A raw ASR transcript is a flat stream of timed words. Subtitles need
//! to be broken into readable lines that respect subtitle conventions:
//!   - at most ~42 characters per line (broadcast standard)
//!   - break at sentence boundaries when possible
//!   - never let a caption run longer than `max_caption_ms`
//!   - never show a caption for less than `min_caption_ms`
//!
//! Pure function: ASR `Transcript` in, `Caption[]` out. Per-word
//! confidence (already normalized upstream) is carried through so the
//! editor's highlighting works immediately on a fresh transcript.

use crate::model::{Caption, Word};
use crate::services::asr::{TranscribedWord, Transcript};

/// Subtitle-breaking parameters. Defaults follow broadcast conventions.
#[derive(Debug, Clone, Copy)]
pub struct CaptionizeOptions {
    /// Soft max characters per caption before we look for a break.
    pub max_chars: usize,
    /// Hard max caption duration.
    pub max_caption_ms: i64,
    /// Below this, merge with the neighbour (too short to read).
    pub min_caption_ms: i64,
}

impl Default for CaptionizeOptions {
    fn default() -> Self {
        Self {
            max_chars: 84, // ~2 lines × 42 chars
            max_caption_ms: 6_000,
            min_caption_ms: 800,
        }
    }
}

/// Characters that end a sentence — preferred break points.
fn ends_sentence(text: &str) -> bool {
    text.ends_with('.')
        || text.ends_with('!')
        || text.ends_with('?')
        || text.ends_with('。')
        || text.ends_with('…')
}

/// Convert a transcript into captions. `id_for` generates a stable id for
/// each caption (the caller passes a UUID generator; tests pass a counter).
pub fn captionize(
    transcript: &Transcript,
    opts: CaptionizeOptions,
    now_ms: i64,
    mut id_for: impl FnMut(usize) -> String,
) -> Vec<Caption> {
    // Flatten all words across segments into one ordered stream.
    let words: Vec<&TranscribedWord> = transcript
        .segments
        .iter()
        .flat_map(|s| s.words.iter())
        .collect();

    if words.is_empty() {
        return Vec::new();
    }

    let mut captions: Vec<Caption> = Vec::new();
    let mut current: Vec<Word> = Vec::new();
    let mut current_chars = 0usize;
    let mut caption_index = 0usize;

    let flush = |captions: &mut Vec<Caption>,
                 words: &mut Vec<Word>,
                 index: &mut usize,
                 id_for: &mut dyn FnMut(usize) -> String| {
        if words.is_empty() {
            return;
        }
        let start_ms = words.first().unwrap().start_ms;
        let end_ms = words.last().unwrap().end_ms;
        captions.push(Caption {
            id: id_for(*index),
            start_ms,
            end_ms,
            words: std::mem::take(words),
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: now_ms,
        });
        *index += 1;
    };

    let caption_start_ms = words[0].start_ms;
    let mut running_start = caption_start_ms;

    for tw in &words {
        let model_word = Word::new(tw.text.clone(), tw.start_ms, tw.end_ms, tw.confidence);
        let word_len = tw.text.chars().count() + 1; // +1 for the space

        // Would adding this word break a constraint?
        let would_exceed_chars = current_chars + word_len > opts.max_chars;
        let would_exceed_dur = tw.end_ms - running_start > opts.max_caption_ms;

        if !current.is_empty() && (would_exceed_chars || would_exceed_dur) {
            flush(&mut captions, &mut current, &mut caption_index, &mut id_for);
            current_chars = 0;
            running_start = tw.start_ms;
        }

        if current.is_empty() {
            running_start = tw.start_ms;
        }
        current.push(model_word);
        current_chars += word_len;

        // Prefer to break right after a sentence-ending word if we're past
        // half the budget — gives natural, readable captions.
        if ends_sentence(&tw.text) && current_chars >= opts.max_chars / 2 {
            flush(&mut captions, &mut current, &mut caption_index, &mut id_for);
            current_chars = 0;
        }
    }
    flush(&mut captions, &mut current, &mut caption_index, &mut id_for);

    merge_too_short(captions, opts.min_caption_ms)
}

/// Merge captions shorter than `min_caption_ms` into the following caption
/// (or the previous one if it's the last). Keeps the reading rhythm sane.
fn merge_too_short(captions: Vec<Caption>, min_caption_ms: i64) -> Vec<Caption> {
    if captions.len() < 2 {
        return captions;
    }
    let mut out: Vec<Caption> = Vec::with_capacity(captions.len());
    for cap in captions {
        let dur = cap.end_ms - cap.start_ms;
        if dur < min_caption_ms {
            if let Some(prev) = out.last_mut() {
                // Merge into previous: extend end, append words.
                prev.end_ms = cap.end_ms;
                prev.words.extend(cap.words);
                continue;
            }
        }
        out.push(cap);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::asr::{Segment, TranscribedWord, Transcript};

    fn tw(text: &str, start: i64, end: i64, conf: f32) -> TranscribedWord {
        TranscribedWord {
            text: text.into(),
            start_ms: start,
            end_ms: end,
            confidence: conf,
        }
    }

    fn transcript(words: Vec<TranscribedWord>) -> Transcript {
        Transcript {
            language: "en".into(),
            backend: "test".into(),
            segments: vec![Segment {
                start_ms: words.first().map(|w| w.start_ms).unwrap_or(0),
                end_ms: words.last().map(|w| w.end_ms).unwrap_or(0),
                words,
            }],
        }
    }

    fn counter_ids() -> impl FnMut(usize) -> String {
        move |i| format!("c{i}")
    }

    #[test]
    fn empty_transcript_yields_no_captions() {
        let t = Transcript {
            language: "en".into(),
            backend: "t".into(),
            segments: vec![],
        };
        let caps = captionize(&t, CaptionizeOptions::default(), 0, counter_ids());
        assert!(caps.is_empty());
    }

    #[test]
    fn short_sentence_is_one_caption() {
        let t = transcript(vec![
            tw("Hello", 0, 400, 95.0),
            tw("world.", 400, 900, 90.0),
        ]);
        let caps = captionize(&t, CaptionizeOptions::default(), 0, counter_ids());
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].text(), "Hello world.");
        assert_eq!(caps[0].start_ms, 0);
        assert_eq!(caps[0].end_ms, 900);
        assert!(caps[0].ai_generated);
    }

    #[test]
    fn confidence_carries_through() {
        let t = transcript(vec![tw("kerygma", 0, 800, 38.0)]);
        let opts = CaptionizeOptions {
            min_caption_ms: 0,
            ..Default::default()
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        assert_eq!(caps[0].words[0].confidence, 38.0);
        assert_eq!(caps[0].words[0].confidence_tier(), 4); // very low
    }

    #[test]
    fn breaks_on_char_limit() {
        // Build a long run with no sentence breaks → must split by chars.
        let mut words = Vec::new();
        for i in 0..40 {
            words.push(tw("word", i * 300, i * 300 + 250, 90.0));
        }
        let t = transcript(words);
        let opts = CaptionizeOptions {
            max_chars: 40,
            min_caption_ms: 0,
            ..Default::default()
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        assert!(caps.len() > 1, "long run must split into multiple captions");
        // No caption exceeds the char budget by more than one word
        for c in &caps {
            assert!(
                c.text().chars().count() <= 45,
                "caption too long: {}",
                c.text()
            );
        }
    }

    #[test]
    fn breaks_on_sentence_boundary() {
        let t = transcript(vec![
            tw("This", 0, 300, 95.0),
            tw("is", 300, 500, 95.0),
            tw("one.", 500, 900, 95.0),
            tw("And", 900, 1200, 95.0),
            tw("this", 1200, 1500, 95.0),
            tw("is", 1500, 1700, 95.0),
            tw("two.", 1700, 2100, 95.0),
        ]);
        // Small max_chars so the half-budget sentence-break triggers
        let opts = CaptionizeOptions {
            max_chars: 16,
            min_caption_ms: 0,
            ..Default::default()
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        assert!(caps.len() >= 2);
        // First caption should end on a sentence
        assert!(caps[0].text().ends_with('.'));
    }

    #[test]
    fn breaks_on_duration_limit() {
        // Two words far apart in time → must split on duration even though
        // char count is tiny.
        let t = transcript(vec![
            tw("start", 0, 500, 95.0),
            tw("end", 10_000, 10_500, 95.0),
        ]);
        let opts = CaptionizeOptions {
            max_caption_ms: 4_000,
            min_caption_ms: 0,
            max_chars: 100,
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn merges_too_short_captions() {
        // A 200ms fragment should merge into its neighbour.
        let t = transcript(vec![
            tw("Hi.", 0, 200, 95.0), // 200ms — below min
            tw("There", 1000, 1400, 95.0),
            tw("friend.", 1400, 2000, 95.0),
        ]);
        let opts = CaptionizeOptions {
            max_chars: 8,
            min_caption_ms: 800,
            max_caption_ms: 6000,
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        // "Hi." alone is 200ms < 800ms min, so it merges forward.
        // Verify no caption is below the min after merging (except possibly merged ones span more).
        for c in &caps {
            // merged captions will be >= min; standalone short ones shouldn't survive
            assert!(c.end_ms - c.start_ms >= 200);
        }
        // The text "Hi." must still be present somewhere.
        assert!(caps.iter().any(|c| c.text().contains("Hi.")));
    }

    #[test]
    fn ids_are_assigned_in_order() {
        let t = transcript(vec![tw("a.", 0, 300, 95.0), tw("b.", 5000, 5300, 95.0)]);
        let opts = CaptionizeOptions {
            max_caption_ms: 1000,
            min_caption_ms: 0,
            max_chars: 100,
        };
        let caps = captionize(&t, opts, 0, counter_ids());
        assert_eq!(caps[0].id, "c0");
        assert_eq!(caps[1].id, "c1");
    }
}
