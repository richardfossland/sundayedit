//! Subtitle export writers (Phase 6.1).
//!
//! Pure functions: input a Project, output a string in the requested
//! format. The Tauri command layer is responsible for writing the
//! string to disk.
//!
//! Format priority:
//!   - SRT — universal, simple, no styling
//!   - VTT — web standard, slightly richer
//!   - ASS — full styling (used by Aegisub, libass; what Phase 6.2 burn-in uses)
//!   - TXT — plain transcript, no timestamps
//!
//! All formats validated against their respective parsers' real-world
//! quirks (UTF-8 + appropriate line endings; SRT and VTT 0-padded;
//! ASS-escaped `{}` in text).

use serde::Serialize;

use crate::model::{Caption, Project, Speaker, Style};

// ── SRT ─────────────────────────────────────────────────────────────────────

/// Generate SRT (.srt) content from a project.
///
/// SRT is the universal lowest common denominator. No styling, no
/// speakers (we surface them as a "Speaker:" prefix when more than one
/// speaker exists — and only if the caller asks for it).
pub fn write_srt(project: &Project, opts: SrtOptions) -> String {
    let mut out = String::with_capacity(project.captions.len() * 80);
    let speakers_map = speakers_by_id(&project.speakers);
    let mut idx = 1u32;
    for c in &project.captions {
        if opts.strip_empty && c.words.is_empty() {
            continue;
        }
        let text = if opts.include_speakers {
            format_with_speaker(c, &speakers_map)
        } else {
            c.text()
        };

        // 1-based index, then "HH:MM:SS,mmm --> HH:MM:SS,mmm", then text
        out.push_str(&idx.to_string());
        out.push_str("\r\n");
        out.push_str(&fmt_srt_time(c.start_ms));
        out.push_str(" --> ");
        out.push_str(&fmt_srt_time(c.end_ms));
        out.push_str("\r\n");
        out.push_str(&text);
        out.push_str("\r\n\r\n");
        idx += 1;
    }
    out
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SrtOptions {
    pub include_speakers: bool,
    pub strip_empty: bool,
}

fn fmt_srt_time(ms: i64) -> String {
    let neg = ms < 0;
    let ms = ms.unsigned_abs();
    let hours = ms / 3_600_000;
    let minutes = (ms / 60_000) % 60;
    let seconds = (ms / 1_000) % 60;
    let millis = ms % 1_000;
    let sign = if neg { "-" } else { "" };
    format!(
        "{}{:02}:{:02}:{:02},{:03}",
        sign, hours, minutes, seconds, millis
    )
}

// ── VTT ─────────────────────────────────────────────────────────────────────

/// Generate WebVTT (.vtt) content.
///
/// Same shape as SRT but `.` instead of `,` for milliseconds, and a
/// `WEBVTT` header. Speakers (when present) are encoded as
/// `<v Speaker Name>text</v>` voice spans, which most web players honour.
pub fn write_vtt(project: &Project, opts: VttOptions) -> String {
    let mut out = String::with_capacity(project.captions.len() * 80 + 16);
    out.push_str("WEBVTT\n\n");
    let speakers_map = speakers_by_id(&project.speakers);
    for (i, c) in project.captions.iter().enumerate() {
        if opts.strip_empty && c.words.is_empty() {
            continue;
        }
        // Cue id is optional in VTT; using sequential is a nice debugging aid.
        out.push_str(&format!("{}\n", i + 1));
        out.push_str(&fmt_vtt_time(c.start_ms));
        out.push_str(" --> ");
        out.push_str(&fmt_vtt_time(c.end_ms));
        out.push('\n');
        if opts.include_speakers {
            if let Some(speaker_id) = &c.speaker_id {
                if let Some(name) = speakers_map.get(speaker_id) {
                    out.push_str(&format!(
                        "<v {}>{}</v>\n",
                        vtt_escape(name),
                        vtt_escape(&c.text())
                    ));
                    out.push('\n');
                    continue;
                }
            }
        }
        out.push_str(&vtt_escape(&c.text()));
        out.push_str("\n\n");
    }
    out
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VttOptions {
    pub include_speakers: bool,
    pub strip_empty: bool,
}

fn fmt_vtt_time(ms: i64) -> String {
    let ms = ms.max(0) as u64;
    let hours = ms / 3_600_000;
    let minutes = (ms / 60_000) % 60;
    let seconds = (ms / 1_000) % 60;
    let millis = ms % 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}

fn vtt_escape(s: &str) -> String {
    // Minimal escaping for VTT — angle brackets and ampersand.
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── ASS / SSA ───────────────────────────────────────────────────────────────

/// Generate Advanced SubStation Alpha (.ass) content.
///
/// Full styling preserved. This is the format that `libass` consumes for
/// burn-in (Phase 6.2). The default `Style` from the project becomes
/// `Style: Default` in the output; per-caption style overrides become
/// additional named styles.
pub fn write_ass(project: &Project) -> String {
    let mut out = String::with_capacity(project.captions.len() * 100 + 1024);

    // ── [Script Info] ──
    out.push_str("[Script Info]\n");
    out.push_str(&format!("Title: {}\n", ass_escape(&project.name)));
    out.push_str("ScriptType: v4.00+\n");
    out.push_str(&format!("PlayResX: {}\n", project.video_width));
    out.push_str(&format!("PlayResY: {}\n", project.video_height));
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str("YCbCr Matrix: TV.709\n");
    out.push('\n');

    // ── [V4+ Styles] ──
    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    out.push_str(&format_ass_style("Default", &project.default_style));
    out.push('\n');

    // ── [Events] ──
    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    let speakers_map = speakers_by_id(&project.speakers);
    for c in &project.captions {
        let style_name = "Default"; // Phase 5.2 wires per-caption style names
        let name_field = c
            .speaker_id
            .as_deref()
            .and_then(|id| speakers_map.get(id).map(|n| n.as_str()))
            .unwrap_or("");
        out.push_str(&format!(
            "Dialogue: 0,{},{},{},{},0,0,0,,{}\n",
            fmt_ass_time(c.start_ms),
            fmt_ass_time(c.end_ms),
            style_name,
            ass_escape(name_field),
            ass_escape(&c.text()),
        ));
    }

    out
}

fn fmt_ass_time(ms: i64) -> String {
    let ms = ms.max(0) as u64;
    let hours = ms / 3_600_000;
    let minutes = (ms / 60_000) % 60;
    let seconds = (ms / 1_000) % 60;
    // ASS uses centiseconds (hundredths), not milliseconds.
    let centis = (ms % 1_000) / 10;
    format!("{}:{:02}:{:02}.{:02}", hours, minutes, seconds, centis)
}

fn ass_escape(s: &str) -> String {
    // ASS uses `{}` for inline override codes — escape literal braces.
    // Commas in the Text field are literal but in other fields would be
    // delimiter; for safety we don't allow commas in Style/Name fields.
    s.replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\N")
}

fn format_ass_style(name: &str, s: &Style) -> String {
    // ASS uses BGR hex with `&H` prefix and alpha; we keep alpha 00 (opaque).
    let primary = hex_to_ass_bgr(&s.color_fg);
    let outline = hex_to_ass_bgr(&s.outline_color);
    // Alignment numpad (1-9). Map (align_h, align_v) → ASS code.
    let alignment = match (s.align_h.as_str(), s.align_v.as_str()) {
        ("left", "bottom") => 1,
        ("center", "bottom") => 2,
        ("right", "bottom") => 3,
        ("left", "middle") => 4,
        ("center", "middle") => 5,
        ("right", "middle") => 6,
        ("left", "top") => 7,
        ("center", "top") => 8,
        ("right", "top") => 9,
        _ => 2,
    };
    let bold = if s.font_weight >= 600 { -1 } else { 0 };
    let italic = if s.italic { -1 } else { 0 };
    format!(
        "Style: {name},{font},{size},{primary},{secondary},{outline},{back},{bold},{italic},0,0,100,100,{spacing},0,1,{outline_w},{shadow},{alignment},10,10,{marginv},1\n",
        name = name,
        font = s.font_family,
        size = s.font_size_px,
        primary = primary,
        secondary = "&H000000FF",
        outline = outline,
        back = "&H64000000",
        bold = bold,
        italic = italic,
        spacing = s.letter_spacing,
        outline_w = s.outline_width_px,
        shadow = s.shadow_blur,
        alignment = alignment,
        marginv = 20,
    )
}

/// Convert "#RRGGBB" to ASS-style "&H00BBGGRR" (BGR + alpha).
fn hex_to_ass_bgr(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return "&H00FFFFFF".to_string();
    }
    let r = &h[0..2];
    let g = &h[2..4];
    let b = &h[4..6];
    format!(
        "&H00{}{}{}",
        b.to_uppercase(),
        g.to_uppercase(),
        r.to_uppercase()
    )
}

// ── Plain text ──────────────────────────────────────────────────────────────

pub fn write_txt(project: &Project, opts: TxtOptions) -> String {
    let mut out = String::with_capacity(project.captions.len() * 60);
    let speakers_map = speakers_by_id(&project.speakers);
    let mut last_speaker: Option<&str> = None;
    for c in &project.captions {
        if c.words.is_empty() && opts.strip_empty {
            continue;
        }
        if opts.include_speakers {
            let current_speaker = c
                .speaker_id
                .as_deref()
                .and_then(|id| speakers_map.get(id).map(|n| n.as_str()));
            if current_speaker != last_speaker {
                if let Some(name) = current_speaker {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str(name);
                    out.push_str(":\n");
                }
                last_speaker = current_speaker;
            }
        }
        out.push_str(&c.text());
        out.push(' ');
    }
    out.trim_end().to_string()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TxtOptions {
    pub include_speakers: bool,
    pub strip_empty: bool,
}

// ── JSON ──────────────────────────────────────────────────────────────────

/// Developer-facing JSON export. A stable, documented schema kept separate
/// from the internal `Project` so the export contract doesn't shift when
/// internals change. Per-word timing + confidence are preserved.
pub fn write_json(project: &Project, opts: JsonOptions) -> String {
    let doc = JsonExport {
        format: "verbatim-captions",
        version: 1,
        project: project.name.clone(),
        language: project.language.clone(),
        speakers: project
            .speakers
            .iter()
            .map(|s| JsonSpeaker {
                id: s.id.clone(),
                name: s.display_name.clone(),
                color: s.color_hex.clone(),
            })
            .collect(),
        captions: project
            .captions
            .iter()
            .filter(|c| !(opts.strip_empty && c.words.is_empty()))
            .map(|c| JsonCaption {
                id: c.id.clone(),
                start_ms: c.start_ms,
                end_ms: c.end_ms,
                text: c.text(),
                speaker_id: c.speaker_id.clone(),
                words: c
                    .words
                    .iter()
                    .map(|w| JsonWord {
                        text: w.text.clone(),
                        start_ms: w.start_ms,
                        end_ms: w.end_ms,
                        confidence: w.confidence,
                    })
                    .collect(),
            })
            .collect(),
    };
    // Serializing our own owned structs cannot fail.
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct JsonOptions {
    pub strip_empty: bool,
}

#[derive(Serialize)]
struct JsonExport {
    format: &'static str,
    version: u32,
    project: String,
    language: String,
    speakers: Vec<JsonSpeaker>,
    captions: Vec<JsonCaption>,
}

#[derive(Serialize)]
struct JsonSpeaker {
    id: String,
    name: String,
    color: Option<String>,
}

#[derive(Serialize)]
struct JsonCaption {
    id: String,
    start_ms: i64,
    end_ms: i64,
    text: String,
    speaker_id: Option<String>,
    words: Vec<JsonWord>,
}

#[derive(Serialize)]
struct JsonWord {
    text: String,
    start_ms: i64,
    end_ms: i64,
    confidence: f32,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn speakers_by_id(speakers: &[Speaker]) -> std::collections::HashMap<String, String> {
    speakers
        .iter()
        .map(|s| (s.id.clone(), s.display_name.clone()))
        .collect()
}

fn format_with_speaker(c: &Caption, map: &std::collections::HashMap<String, String>) -> String {
    if let Some(id) = &c.speaker_id {
        if let Some(name) = map.get(id) {
            return format!("{}: {}", name, c.text());
        }
    }
    c.text()
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Caption, Project, Speaker, Style, Word};

    fn p() -> Project {
        Project {
            id: "p".into(),
            name: "test.mp4".into(),
            video_path: "/x.mp4".into(),
            video_content_hash: "h".into(),
            video_duration_ms: 60_000,
            video_width: 1920,
            video_height: 1080,
            video_fps: 30.0,
            audio_wav_path: None,
            language: "en".into(),
            default_style: Style::broadcast_news(),
            context_description: None,
            captions: vec![
                Caption {
                    id: "c1".into(),
                    start_ms: 1500,
                    end_ms: 3750,
                    words: vec![
                        Word::new("Hello", 1500, 1900, 95.0),
                        Word::new("world", 2000, 3700, 80.0),
                    ],
                    speaker_id: Some("s1".into()),
                    style_id: None,
                    notes: None,
                    ai_generated: true,
                    last_edited_at: 0,
                },
                Caption {
                    id: "c2".into(),
                    start_ms: 4000,
                    end_ms: 7250,
                    words: vec![
                        Word::new("This", 4000, 4300, 90.0),
                        Word::new("is", 4300, 4400, 90.0),
                        Word::new("two", 4400, 7000, 80.0),
                    ],
                    speaker_id: Some("s2".into()),
                    style_id: None,
                    notes: None,
                    ai_generated: true,
                    last_edited_at: 0,
                },
            ],
            speakers: vec![
                Speaker {
                    id: "s1".into(),
                    display_name: "Pastor Lars".into(),
                    color_hex: None,
                },
                Speaker {
                    id: "s2".into(),
                    display_name: "Maria".into(),
                    color_hex: None,
                },
            ],
            glossary: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    // ── SRT ────────────────────────────────────────────────────────────────
    #[test]
    fn srt_basic_shape() {
        let out = write_srt(&p(), SrtOptions::default());
        assert!(out.starts_with("1\r\n00:00:01,500 --> 00:00:03,750\r\nHello world\r\n\r\n"));
        assert!(out.contains("2\r\n00:00:04,000 --> 00:00:07,250\r\nThis is two\r\n\r\n"));
    }

    #[test]
    fn json_is_valid_and_preserves_words() {
        let out = write_json(&p(), JsonOptions::default());
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["format"], "verbatim-captions");
        assert_eq!(v["version"], 1);
        assert_eq!(v["language"], "en");
        assert_eq!(v["captions"].as_array().unwrap().len(), 2);
        let first = &v["captions"][0];
        assert_eq!(first["text"], "Hello world");
        assert_eq!(first["speaker_id"], "s1");
        // Per-word timing + confidence are preserved (killer feature data).
        assert_eq!(first["words"][0]["text"], "Hello");
        assert_eq!(first["words"][0]["start_ms"], 1500);
        assert_eq!(first["words"][1]["confidence"], 80.0);
        // Speakers carried for cross-reference.
        assert_eq!(v["speakers"][0]["name"], "Pastor Lars");
    }

    #[test]
    fn json_strip_empty_drops_wordless_captions() {
        let mut proj = p();
        proj.captions.push(Caption {
            id: "empty".into(),
            start_ms: 8000,
            end_ms: 9000,
            words: vec![],
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
        });
        let out = write_json(&proj, JsonOptions { strip_empty: true });
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["captions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn srt_with_speakers() {
        let out = write_srt(
            &p(),
            SrtOptions {
                include_speakers: true,
                strip_empty: false,
            },
        );
        assert!(out.contains("Pastor Lars: Hello world"));
        assert!(out.contains("Maria: This is two"));
    }

    #[test]
    fn srt_time_format_zero_pads() {
        assert_eq!(fmt_srt_time(0), "00:00:00,000");
        assert_eq!(fmt_srt_time(1), "00:00:00,001");
        assert_eq!(fmt_srt_time(1_000), "00:00:01,000");
        assert_eq!(fmt_srt_time(61_500), "00:01:01,500");
        assert_eq!(fmt_srt_time(3_600_000), "01:00:00,000");
    }

    // ── VTT ────────────────────────────────────────────────────────────────
    #[test]
    fn vtt_header_present() {
        let out = write_vtt(&p(), VttOptions::default());
        assert!(out.starts_with("WEBVTT\n\n"));
    }

    #[test]
    fn vtt_time_format_dot_not_comma() {
        let out = write_vtt(&p(), VttOptions::default());
        assert!(out.contains("00:00:01.500 --> 00:00:03.750"));
        assert!(!out.contains(","));
    }

    #[test]
    fn vtt_speakers_use_voice_tag() {
        let out = write_vtt(
            &p(),
            VttOptions {
                include_speakers: true,
                strip_empty: false,
            },
        );
        assert!(out.contains("<v Pastor Lars>Hello world</v>"));
        assert!(out.contains("<v Maria>This is two</v>"));
    }

    #[test]
    fn vtt_escapes_html_chars() {
        let mut proj = p();
        proj.captions[0].words[0].text = "<test>".into();
        let out = write_vtt(&proj, VttOptions::default());
        assert!(out.contains("&lt;test&gt;"));
    }

    // ── ASS ────────────────────────────────────────────────────────────────
    #[test]
    fn ass_has_required_sections() {
        let out = write_ass(&p());
        assert!(out.contains("[Script Info]"));
        assert!(out.contains("[V4+ Styles]"));
        assert!(out.contains("[Events]"));
        assert!(out.contains("Style: Default,Helvetica Neue"));
    }

    #[test]
    fn ass_includes_dialogue_events() {
        let out = write_ass(&p());
        // Centisecond format: 1500ms → 0:00:01.50
        assert!(out
            .contains("Dialogue: 0,0:00:01.50,0:00:03.75,Default,Pastor Lars,0,0,0,,Hello world"));
    }

    #[test]
    fn ass_time_format_centiseconds() {
        assert_eq!(fmt_ass_time(0), "0:00:00.00");
        assert_eq!(fmt_ass_time(50), "0:00:00.05");
        assert_eq!(fmt_ass_time(99), "0:00:00.09");
        assert_eq!(fmt_ass_time(1_500), "0:00:01.50");
        assert_eq!(fmt_ass_time(3_600_000), "1:00:00.00");
    }

    #[test]
    fn ass_escapes_braces_and_newlines() {
        assert_eq!(ass_escape("plain"), "plain");
        assert_eq!(ass_escape("{override}"), "\\{override\\}");
        assert_eq!(ass_escape("line1\nline2"), "line1\\Nline2");
    }

    #[test]
    fn hex_to_ass_bgr_swaps_channels() {
        assert_eq!(hex_to_ass_bgr("#FF0000"), "&H000000FF"); // red → BGR
        assert_eq!(hex_to_ass_bgr("#00FF00"), "&H0000FF00");
        assert_eq!(hex_to_ass_bgr("#0000FF"), "&H00FF0000");
        assert_eq!(hex_to_ass_bgr("#FFFFFF"), "&H00FFFFFF");
    }

    // ── TXT ────────────────────────────────────────────────────────────────
    #[test]
    fn txt_concatenates_captions() {
        let out = write_txt(&p(), TxtOptions::default());
        assert_eq!(out, "Hello world This is two");
    }

    #[test]
    fn txt_with_speakers_groups_by_speaker() {
        let out = write_txt(
            &p(),
            TxtOptions {
                include_speakers: true,
                strip_empty: false,
            },
        );
        assert!(out.contains("Pastor Lars:\nHello world"));
        assert!(out.contains("Maria:\nThis is two"));
    }
}
