//! Core caption domain model.
//!
//! All operations on Project/Caption/Word live in `services::operations`
//! and are PURE FUNCTIONS — they take a state and return a new state,
//! never mutate in place. This makes undo trivial (keep the previous
//! state) and the model easy to reason about.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Word ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Word.ts")]
pub struct Word {
    pub text: String,
    // i64 in Rust, but Tauri serializes it as a JSON number — tell ts-rs to
    // emit `number` (not `bigint`) so the wire format and the type agree.
    #[ts(type = "number")]
    pub start_ms: i64,
    #[ts(type = "number")]
    pub end_ms: i64,
    /// 0–100 normalized confidence from ASR.
    pub confidence: f32,
    /// User has changed this word from the ASR output.
    pub edited: bool,
    /// User has confirmed — do not surface as uncertain even if confidence is low.
    pub locked: bool,
    /// AI polish (Phase 4.1) adjusted this word's punctuation/casing. Not a
    /// content change, so it does NOT trust the word like `edited` does —
    /// it only drives the "polished" dot in the editor.
    #[serde(default)]
    pub polished: bool,
    /// Top alternates from ASR (max 3).
    #[serde(default)]
    pub alternates: Vec<AlternateRead>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/AlternateRead.ts")]
pub struct AlternateRead {
    pub text: String,
    pub confidence: f32,
}

impl Word {
    pub fn new(text: impl Into<String>, start_ms: i64, end_ms: i64, confidence: f32) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms,
            confidence,
            edited: false,
            locked: false,
            polished: false,
            alternates: Vec::new(),
        }
    }

    /// Per-product convention — see `docs/ARCHITECTURE.md` confidence-tier
    /// table. Tier 1 = high confidence (don't touch). Tier 4 = very low
    /// (demands attention).
    pub fn confidence_tier(&self) -> u8 {
        if self.locked || self.edited {
            return 1;
        }
        match self.confidence {
            c if c >= 85.0 => 1,
            c if c >= 70.0 => 2,
            c if c >= 50.0 => 3,
            _ => 4,
        }
    }
}

// ── Caption ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Caption.ts")]
pub struct Caption {
    pub id: String,
    #[ts(type = "number")]
    pub start_ms: i64,
    #[ts(type = "number")]
    pub end_ms: i64,
    pub words: Vec<Word>,
    pub speaker_id: Option<String>,
    pub style_id: Option<String>,
    pub notes: Option<String>,
    pub ai_generated: bool,
    #[ts(type = "number")]
    pub last_edited_at: i64,
}

impl Caption {
    /// The rendered text — derived from words on read. Kept here for
    /// convenience but never persisted as a separate field; words are
    /// the source of truth.
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Number of words below the given confidence threshold (excluding
    /// locked/edited words — those are trusted).
    pub fn uncertain_word_count(&self, threshold: f32) -> usize {
        self.words
            .iter()
            .filter(|w| !w.locked && !w.edited && w.confidence < threshold)
            .count()
    }
}

// ── Speaker ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Speaker.ts")]
pub struct Speaker {
    pub id: String,
    pub display_name: String,
    pub color_hex: Option<String>,
}

// ── GlossaryTerm ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/GlossaryTerm.ts")]
pub struct GlossaryTerm {
    pub id: String,
    /// Canonical form — what we want in the final output.
    pub term: String,
    /// Likely misrecognitions to auto-correct to `term`.
    pub aliases: Vec<String>,
    pub definition: Option<String>,
    pub pronunciation_hint: Option<String>,
}

// ── Clip ──────────────────────────────────────────────────────────────────────

/// A social-media clip carved out of the talk (Phase: SundayEdit clips).
/// `caption_ids` are the source captions the clip covers; `start_ms`/`end_ms`
/// are derived from those captions' real timings (never model-invented). The
/// `title` is the clip's main point, rendered as a large on-screen overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Clip.ts")]
pub struct Clip {
    pub id: String,
    /// The main point — shown as a large title overlay on the clip.
    pub title: String,
    /// One-line summary / hook for the clip.
    pub hook: String,
    /// Source captions this clip covers.
    pub caption_ids: Vec<String>,
    #[ts(type = "number")]
    pub start_ms: i64,
    #[ts(type = "number")]
    pub end_ms: i64,
}

// ── Style ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Style.ts")]
pub struct Style {
    pub id: String,
    pub name: String,
    pub font_family: String,
    pub font_size_px: i32,
    pub font_weight: i32,
    pub italic: bool,
    pub color_fg: String, // hex
    pub outline_color: String,
    pub outline_width_px: i32,
    pub shadow_color: String,
    pub shadow_offset_x: i32,
    pub shadow_offset_y: i32,
    pub shadow_blur: i32,
    pub background_color: Option<String>,
    pub background_padding_px: i32,
    pub background_radius_px: i32,
    pub align_h: String, // "left" | "center" | "right"
    pub align_v: String, // "top" | "middle" | "bottom"
    pub anchor: String,  // 9-grid: "tl","tc","tr","ml","mc","mr","bl","bc","br"
    pub max_width_pct: f32,
    pub line_spacing: f32,
    pub letter_spacing: f32,
    pub animation: Option<AnimationSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/AnimationSpec.ts")]
pub struct AnimationSpec {
    /// "fade" | "slide_left" | "slide_right" | "slide_up" | "slide_down" |
    /// "karaoke" | "popup" | "none"
    pub kind: String,
    pub duration_ms: i32,
    pub per_word_delay_ms: i32,
}

impl Style {
    /// "Broadcast News" — sober, accessibility-focused. SundayEdit's safe default.
    pub fn broadcast_news() -> Self {
        Self {
            id: "preset:broadcast_news".to_string(),
            name: "Broadcast News".to_string(),
            font_family: "Helvetica Neue".to_string(),
            font_size_px: 42,
            font_weight: 600,
            italic: false,
            color_fg: "#FFFFFF".to_string(),
            outline_color: "#000000".to_string(),
            outline_width_px: 3,
            shadow_color: "#00000080".to_string(),
            shadow_offset_x: 0,
            shadow_offset_y: 2,
            shadow_blur: 6,
            background_color: None,
            background_padding_px: 0,
            background_radius_px: 0,
            align_h: "center".into(),
            align_v: "bottom".into(),
            anchor: "bc".into(),
            max_width_pct: 80.0,
            line_spacing: 1.1,
            letter_spacing: 0.0,
            animation: Some(AnimationSpec {
                kind: "fade".into(),
                duration_ms: 200,
                per_word_delay_ms: 0,
            }),
        }
    }

    /// Title-overlay style for social clips — large, bold, top-centre, so the
    /// clip's main point reads at a glance above the captions.
    pub fn title_overlay() -> Self {
        Self {
            id: "preset:title_overlay".to_string(),
            name: "Title".to_string(),
            font_family: "Helvetica Neue".to_string(),
            font_size_px: 72,
            font_weight: 800,
            italic: false,
            color_fg: "#FFFFFF".to_string(),
            outline_color: "#000000".to_string(),
            outline_width_px: 4,
            shadow_color: "#000000A0".to_string(),
            shadow_offset_x: 0,
            shadow_offset_y: 3,
            shadow_blur: 10,
            background_color: None,
            background_padding_px: 0,
            background_radius_px: 0,
            align_h: "center".into(),
            align_v: "top".into(),
            anchor: "tc".into(),
            max_width_pct: 88.0,
            line_spacing: 1.1,
            letter_spacing: 0.0,
            animation: Some(AnimationSpec {
                kind: "fade".into(),
                duration_ms: 250,
                per_word_delay_ms: 0,
            }),
        }
    }
}

// ── ExportConfig ─────────────────────────────────────────────────────────────

/// Persisted export preferences for sidecar text format + burn-in style.
/// Stored per-project; sane defaults so it's always valid on first use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ExportConfig.ts")]
pub struct ExportConfig {
    /// Default sidecar format: "srt" | "vtt" | "ass"
    pub format: String,
    /// Whether to add captions as burn-in when using a platform preset.
    pub burn_in: bool,
    /// Caption font size in px (16 / 20 / 24 / 28).
    pub caption_size_px: i32,
    /// Caption text colour: "white" | "yellow" | "green"
    pub caption_color: String,
    /// Caption background: "black" | "semitransparent" | "none"
    pub caption_background: String,
    /// Maximum characters per caption line: 32 | 42 | 52
    pub max_chars_per_line: i32,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: "srt".into(),
            burn_in: false,
            caption_size_px: 24,
            caption_color: "white".into(),
            caption_background: "semitransparent".into(),
            max_chars_per_line: 42,
        }
    }
}

// ── ProjectMeta ──────────────────────────────────────────────────────────────

/// User-editable project metadata: title, video description (used as AI
/// context), glossary names for Whisper priming, and preferred language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ProjectMeta.ts")]
pub struct ProjectMeta {
    /// Human-readable title (overrides the bare filename in the UI).
    pub title: String,
    /// Prose description of the video — fed to AI as context.
    pub description: String,
    /// Comma-separated list of proper nouns / glossary hints for Whisper.
    pub proper_nouns: String,
    /// Transcription/translation language: "auto" | ISO 639-1 code
    pub language: String,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            proper_nouns: String::new(),
            language: "auto".into(),
        }
    }
}

// ── Project ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Project.ts")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub video_path: String,
    pub video_content_hash: String,
    #[ts(type = "number")]
    pub video_duration_ms: i64,
    pub video_width: i32,
    pub video_height: i32,
    pub video_fps: f32,
    pub audio_wav_path: Option<String>,
    pub language: String,
    pub default_style: Style,
    pub context_description: Option<String>,
    pub captions: Vec<Caption>,
    pub speakers: Vec<Speaker>,
    pub glossary: Vec<GlossaryTerm>,
    /// AI-generated social clips carved from the talk (SundayEdit).
    #[serde(default)]
    pub clips: Vec<Clip>,
    /// Short AI summary of the whole talk (SundayEdit).
    #[serde(default)]
    pub talk_summary: Option<String>,
    /// Configurable export pipeline settings (format, burn-in, style).
    #[serde(default)]
    pub export_config: ExportConfig,
    /// Editable project metadata (title, description, proper-noun hints).
    #[serde(default)]
    pub project_meta: ProjectMeta,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
}

impl Project {
    /// Validate the invariants documented in `docs/ARCHITECTURE.md`:
    ///   1. Captions never overlap in time
    ///   2. Captions are sorted by start_ms
    ///   3. start < end on every caption
    ///   4. Word ranges within their caption are non-decreasing
    pub fn validate(&self) -> Result<(), String> {
        let mut last_end = i64::MIN;
        for (i, c) in self.captions.iter().enumerate() {
            if c.start_ms >= c.end_ms {
                return Err(format!("caption[{}] has start >= end", i));
            }
            if c.start_ms < last_end {
                return Err(format!(
                    "caption[{}] starts at {} but previous ended at {} — overlap",
                    i, c.start_ms, last_end
                ));
            }
            // words timing
            let mut prev_word_end = c.start_ms;
            for (wi, w) in c.words.iter().enumerate() {
                if w.start_ms < prev_word_end {
                    return Err(format!(
                        "caption[{}].word[{}] starts at {} before previous word end {}",
                        i, wi, w.start_ms, prev_word_end
                    ));
                }
                if w.end_ms <= w.start_ms {
                    return Err(format!("caption[{}].word[{}] has end <= start", i, wi));
                }
                if w.end_ms > c.end_ms {
                    return Err(format!(
                        "caption[{}].word[{}] ends at {} after caption end {}",
                        i, wi, w.end_ms, c.end_ms
                    ));
                }
                prev_word_end = w.end_ms;
            }
            last_end = c.end_ms;
        }
        Ok(())
    }
}
