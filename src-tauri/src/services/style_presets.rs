//! Bundled subtitle style presets — Phase 5.1 + 5.3.
//!
//! A first-time user should be able to produce professional-looking
//! captions in under 60 seconds just by picking a preset. These are
//! hand-tuned `Style` values grouped by use-case. They're pure data, so
//! they're trivially testable and ship in the binary (no download).
//!
//! Every preset drives BOTH the live CSS preview (`style_to_css`) and the
//! ASS burn-in (`export::write_ass`) from the same `Style` fields, which
//! is how we keep "preview == output".

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{AnimationSpec, Style};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/StylePreset.ts")]
pub struct StylePreset {
    pub style: Style,
    /// Grouping for the gallery: "Broadcast", "Social", "Cinema", ...
    pub category: String,
    pub description: String,
}

// Internal preset-builder: a flat positional constructor keeps the preset
// table below readable; not a public API.
#[allow(clippy::too_many_arguments)]
fn style(
    id: &str,
    name: &str,
    font_family: &str,
    font_size_px: i32,
    font_weight: i32,
    color_fg: &str,
    outline_color: &str,
    outline_width_px: i32,
    background_color: Option<&str>,
    anchor: &str,
    align_h: &str,
    align_v: &str,
    animation: Option<AnimationSpec>,
) -> Style {
    Style {
        id: id.to_string(),
        name: name.to_string(),
        font_family: font_family.to_string(),
        font_size_px,
        font_weight,
        italic: false,
        color_fg: color_fg.to_string(),
        outline_color: outline_color.to_string(),
        outline_width_px,
        shadow_color: "#00000099".to_string(),
        shadow_offset_x: 0,
        shadow_offset_y: 2,
        shadow_blur: 6,
        background_color: background_color.map(|s| s.to_string()),
        background_padding_px: if background_color.is_some() { 10 } else { 0 },
        background_radius_px: if background_color.is_some() { 6 } else { 0 },
        align_h: align_h.to_string(),
        align_v: align_v.to_string(),
        anchor: anchor.to_string(),
        max_width_pct: 80.0,
        line_spacing: 1.1,
        letter_spacing: 0.0,
        animation,
    }
}

fn fade() -> Option<AnimationSpec> {
    Some(AnimationSpec {
        kind: "fade".into(),
        duration_ms: 200,
        per_word_delay_ms: 0,
    })
}

/// The full preset catalog for the gallery.
pub fn catalog() -> Vec<StylePreset> {
    vec![
        // ── Broadcast ──
        StylePreset {
            category: "Broadcast".into(),
            description: "Clean, sober, accessibility-focused. The safe default.".into(),
            style: Style::broadcast_news(),
        },
        // ── Education ──
        StylePreset {
            category: "Education".into(),
            description: "Calm and very readable. Generous for longer lines.".into(),
            style: {
                let mut s = style(
                    "preset:education",
                    "Education",
                    "Open Sans",
                    40,
                    500,
                    "#FFFFFF",
                    "#1A1A1A",
                    2,
                    Some("#000000B0"),
                    "bc",
                    "center",
                    "bottom",
                    fade(),
                );
                s.line_spacing = 1.25;
                s.max_width_pct = 86.0;
                s
            },
        },
        // ── Social — TikTok/Reels/Shorts ──
        StylePreset {
            category: "Social".into(),
            description: "Bold, high-contrast, attention-grabbing. Vertical-first.".into(),
            style: style(
                "preset:tiktok_bold",
                "TikTok Bold",
                "Montserrat",
                64,
                800,
                "#FFFFFF",
                "#000000",
                5,
                None,
                "mc",
                "center",
                "middle",
                Some(AnimationSpec {
                    kind: "popup".into(),
                    duration_ms: 150,
                    per_word_delay_ms: 0,
                }),
            ),
        },
        StylePreset {
            category: "Social".into(),
            description: "Yellow word-pop, the classic creator look.".into(),
            style: {
                let mut s = style(
                    "preset:creator_yellow",
                    "Creator Yellow",
                    "Montserrat",
                    58,
                    800,
                    "#FFE600",
                    "#000000",
                    6,
                    None,
                    "mc",
                    "center",
                    "middle",
                    Some(AnimationSpec {
                        kind: "karaoke".into(),
                        duration_ms: 120,
                        per_word_delay_ms: 60,
                    }),
                );
                s.letter_spacing = 0.5;
                s
            },
        },
        // ── Music / Karaoke ──
        StylePreset {
            category: "Music".into(),
            description: "Word-by-word highlight for sing-along.".into(),
            style: style(
                "preset:karaoke",
                "Karaoke",
                "Arial",
                52,
                700,
                "#FFFFFF",
                "#202080",
                4,
                Some("#000000A0"),
                "bc",
                "center",
                "bottom",
                Some(AnimationSpec {
                    kind: "karaoke".into(),
                    duration_ms: 100,
                    per_word_delay_ms: 80,
                }),
            ),
        },
        // ── Cinema ──
        StylePreset {
            category: "Cinema".into(),
            description: "Traditional movie subtitles, lower-third.".into(),
            style: {
                let mut s = style(
                    "preset:cinema",
                    "Cinema Subtitles",
                    "Helvetica Neue",
                    38,
                    500,
                    "#F2F2F2",
                    "#000000",
                    2,
                    None,
                    "bc",
                    "center",
                    "bottom",
                    fade(),
                );
                s.max_width_pct = 70.0;
                s
            },
        },
        // ── Documentary ──
        StylePreset {
            category: "Cinema".into(),
            description: "Subtle, cinematic, restrained.".into(),
            style: style(
                "preset:documentary",
                "Documentary",
                "Georgia",
                36,
                400,
                "#FFFFFF",
                "#000000",
                1,
                Some("#00000080"),
                "bc",
                "center",
                "bottom",
                fade(),
            ),
        },
        // ── Corporate ──
        StylePreset {
            category: "Corporate".into(),
            description: "Professional and restrained for business video.".into(),
            style: style(
                "preset:corporate",
                "Corporate",
                "Inter",
                40,
                600,
                "#FFFFFF",
                "#0F2A54",
                0,
                Some("#0F2A54E0"),
                "bc",
                "center",
                "bottom",
                fade(),
            ),
        },
        // ── Minimal ──
        StylePreset {
            category: "Minimal".into(),
            description: "No outline, no box — just clean type. Use over calm footage.".into(),
            style: style(
                "preset:minimal",
                "Minimal",
                "Inter",
                42,
                500,
                "#FFFFFF",
                "#000000",
                0,
                None,
                "bc",
                "center",
                "bottom",
                fade(),
            ),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Project, Word};
    use crate::services::export::write_ass;

    #[test]
    fn catalog_has_presets() {
        assert!(catalog().len() >= 8);
    }

    #[test]
    fn preset_ids_are_distinct() {
        let cat = catalog();
        let ids: std::collections::HashSet<_> = cat.iter().map(|p| p.style.id.clone()).collect();
        assert_eq!(ids.len(), cat.len(), "every preset has a unique id");
    }

    #[test]
    fn preset_names_are_distinct() {
        let cat = catalog();
        let names: std::collections::HashSet<_> =
            cat.iter().map(|p| p.style.name.clone()).collect();
        assert_eq!(names.len(), cat.len());
    }

    #[test]
    fn every_preset_has_category_and_description() {
        for p in catalog() {
            assert!(!p.category.is_empty(), "{} missing category", p.style.id);
            assert!(
                !p.description.is_empty(),
                "{} missing description",
                p.style.id
            );
        }
    }

    #[test]
    fn anchors_are_valid_9grid() {
        let valid = ["tl", "tc", "tr", "ml", "mc", "mr", "bl", "bc", "br"];
        for p in catalog() {
            assert!(
                valid.contains(&p.style.anchor.as_str()),
                "{} has invalid anchor {}",
                p.style.id,
                p.style.anchor
            );
        }
    }

    #[test]
    fn colors_are_hex() {
        for p in catalog() {
            assert!(
                p.style.color_fg.starts_with('#'),
                "{} fg not hex",
                p.style.id
            );
            assert!(
                p.style.outline_color.starts_with('#'),
                "{} outline not hex",
                p.style.id
            );
        }
    }

    // Each preset must produce a valid ASS style line (burn-in path).
    #[test]
    fn every_preset_renders_to_ass() {
        for p in catalog() {
            let project = Project {
                id: "p".into(),
                name: "t".into(),
                video_path: "/x".into(),
                video_content_hash: "h".into(),
                video_duration_ms: 1000,
                video_width: 1920,
                video_height: 1080,
                video_fps: 30.0,
                audio_wav_path: None,
                language: "en".into(),
                default_style: p.style.clone(),
                context_description: None,
                captions: vec![Caption {
                    id: "c1".into(),
                    start_ms: 0,
                    end_ms: 1000,
                    words: vec![Word::new("Hello", 0, 1000, 95.0)],
                    speaker_id: None,
                    style_id: None,
                    notes: None,
                    ai_generated: true,
                    last_edited_at: 0,
                }],
                speakers: vec![],
                glossary: vec![],
                created_at: 0,
                updated_at: 0,
            };
            let ass = write_ass(&project);
            assert!(
                ass.contains(&format!("Style: Default,{}", p.style.font_family)),
                "preset {} did not render its font into ASS",
                p.style.id
            );
            assert!(ass.contains("Dialogue:"));
        }
    }
}
