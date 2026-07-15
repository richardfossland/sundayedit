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

use crate::error::{AppError, AppResult};
use crate::model::{Caption, Clip, Project, Speaker, Style};

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
        let text = sanitize_cue_text(&if opts.include_speakers {
            format_with_speaker(c, &speakers_map)
        } else {
            c.text()
        });

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
    let mut idx = 1u32;
    for c in &project.captions {
        if opts.strip_empty && c.words.is_empty() {
            continue;
        }
        // Cue id is optional in VTT; a contiguous 1-based counter (incremented
        // only on emit, like the SRT writer) is a nice debugging aid — using the
        // raw enumerate index would leave gaps where `strip_empty` dropped cues.
        out.push_str(&format!("{idx}\n"));
        idx += 1;
        out.push_str(&fmt_vtt_time(c.start_ms));
        out.push_str(" --> ");
        out.push_str(&fmt_vtt_time(c.end_ms));
        out.push('\n');
        if opts.include_speakers {
            if let Some(speaker_id) = &c.speaker_id {
                if let Some(name) = speakers_map.get(speaker_id) {
                    out.push_str(&format!(
                        "<v {}>{}</v>\n",
                        vtt_escape(&sanitize_cue_text(name)),
                        vtt_escape(&sanitize_cue_text(&c.text()))
                    ));
                    out.push('\n');
                    continue;
                }
            }
        }
        out.push_str(&vtt_escape(&sanitize_cue_text(&c.text())));
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

/// Neutralise characters in caption/speaker text that would break SRT/VTT cue
/// framing. Both formats delimit cues with a BLANK LINE, so an embedded blank
/// line (from a multi-line word, e.g. a pasted find/replace value) truncates
/// the cue and desynchronises every following entry. Carriage returns are also
/// dropped so a stray '\r' can't forge an early line end. A single line break
/// inside a cue is legal (multi-line caption) and is preserved as a lone '\n';
/// runs of blank lines collapse to one break.
fn sanitize_cue_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_newline = false;
    for ch in s.chars() {
        match ch {
            '\r' => {} // drop CRs; never a meaningful in-cue character
            '\n' => {
                if !last_was_newline {
                    out.push('\n');
                    last_was_newline = true;
                }
                // collapse consecutive newlines (the blank line that breaks cues)
            }
            _ => {
                out.push(ch);
                last_was_newline = false;
            }
        }
    }
    out
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
            ass_field(name_field), // Name is comma-delimited — must not contain a raw comma
            ass_escape(&c.text()), // Text is the trailing field — commas stay literal
        ));
    }

    out
}

/// Generate ASS for a single social clip (SundayEdit), for burn-in into a
/// vertical export. Two differences from `write_ass`:
///   1. Caption timings are offset to clip-relative 0, because ffmpeg `-ss`
///      trims the input so the rendered clip's timeline starts at 0.
///   2. A second `Title` style renders the clip's main-point overlay, on a
///      higher layer, spanning the whole clip.
///
/// `play_res_w/h` are the OUTPUT (vertical) dimensions so libass sizes the
/// title for the target frame.
pub fn write_clip_ass(
    project: &Project,
    clip: &Clip,
    title_style: &Style,
    play_res_w: i32,
    play_res_h: i32,
) -> String {
    let clip_dur = (clip.end_ms - clip.start_ms).max(1);
    let mut out = String::with_capacity(512);

    out.push_str("[Script Info]\n");
    out.push_str(&format!("Title: {}\n", ass_escape(&clip.title)));
    out.push_str("ScriptType: v4.00+\n");
    out.push_str(&format!("PlayResX: {play_res_w}\n"));
    out.push_str(&format!("PlayResY: {play_res_h}\n"));
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n");
    out.push_str("YCbCr Matrix: TV.709\n");
    out.push('\n');

    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    out.push_str(&format_ass_style("Default", &project.default_style));
    out.push_str(&format_ass_style("Title", title_style));
    out.push('\n');

    out.push_str("[Events]\n");
    out.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    // Caption events overlapping the clip, offset to clip-relative time.
    for c in &project.captions {
        if c.end_ms <= clip.start_ms || c.start_ms >= clip.end_ms {
            continue;
        }
        let start = (c.start_ms - clip.start_ms).max(0);
        let end = (c.end_ms - clip.start_ms).min(clip_dur);
        if end <= start {
            continue;
        }
        out.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            fmt_ass_time(start),
            fmt_ass_time(end),
            ass_escape(&c.text()),
        ));
    }

    // The clip's main point as a title overlay spanning the whole clip.
    if !clip.title.trim().is_empty() {
        out.push_str(&format!(
            "Dialogue: 1,{},{},Title,,0,0,0,,{}\n",
            fmt_ass_time(0),
            fmt_ass_time(clip_dur),
            ass_escape(&clip.title),
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
    // ASS uses `{}` for inline override codes — escape literal braces. Newlines
    // become `\N`. Commas are left intact: this is used for the trailing Text
    // field (and freeform Title), where a comma is a literal character. Use
    // [`ass_field`] for any earlier comma-DELIMITED field (Name, Fontname).
    s.replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\N")
}

/// Escape a value destined for a comma-DELIMITED ASS field (`Name`, `Fontname`).
/// Same as [`ass_escape`] but also neutralizes commas → a speaker name like
/// "Smith, Jr." or a font with a comma would otherwise shift every following
/// field and corrupt the `Dialogue:`/`Style:` line.
fn ass_field(s: &str) -> String {
    ass_escape(s).replace(',', " ")
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
        font = ass_field(&s.font_family), // Fontname is comma-delimited
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
    // Need ≥6 ASCII hex digits. A non-conforming value (too short, or a multibyte
    // char inside the first 6 bytes from a hand-edited/migrated project file) must
    // fall back to white, NOT panic on a char-boundary byte-slice.
    if h.len() < 6 || !h.as_bytes()[..6].iter().all(u8::is_ascii_hexdigit) {
        return "&H00FFFFFF".to_string();
    }
    // First 6 bytes are ASCII hex (1 byte each) → these slices are on char boundaries.
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
        format: "sundayedit-captions",
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

// ── DOCX ──────────────────────────────────────────────────────────────────

const DOCX_CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
</Types>";

const DOCX_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
</Relationships>";

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn docx_paragraph(text: &str) -> String {
    format!(
        "<w:p><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(text)
    )
}

/// Build a minimal but valid .docx (OOXML) for non-technical review: the
/// project name as a heading, then one paragraph per caption (optional
/// "Speaker:" prefix). Returns the zip bytes. Pure — writes to an in-memory
/// buffer — so it's testable offline.
pub fn build_docx(project: &Project, opts: TxtOptions) -> AppResult<Vec<u8>> {
    use std::io::Write;

    let speakers = speakers_by_id(&project.speakers);
    let mut body = docx_paragraph(&project.name);
    for c in &project.captions {
        if opts.strip_empty && c.words.is_empty() {
            continue;
        }
        let text = if opts.include_speakers {
            format_with_speaker(c, &speakers)
        } else {
            c.text()
        };
        body.push_str(&docx_paragraph(&text));
    }

    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:body>{body}<w:sectPr/></w:body></w:document>"
    );

    let map_zip = |e: zip::result::ZipError| AppError::Internal(format!("docx zip: {e}"));
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zw.start_file("[Content_Types].xml", opt).map_err(map_zip)?;
    zw.write_all(DOCX_CONTENT_TYPES.as_bytes())?;
    zw.start_file("_rels/.rels", opt).map_err(map_zip)?;
    zw.write_all(DOCX_RELS.as_bytes())?;
    zw.start_file("word/document.xml", opt).map_err(map_zip)?;
    zw.write_all(document.as_bytes())?;

    Ok(zw.finish().map_err(map_zip)?.into_inner())
}

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
                    track_id: None,
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
                    track_id: None,
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

    // ── Clip ASS ───────────────────────────────────────────────────────────
    #[test]
    fn clip_ass_offsets_captions_and_adds_title() {
        use crate::model::Clip;
        let clip = Clip {
            id: "clip:0".into(),
            title: "Grace".into(),
            hook: "h".into(),
            caption_ids: vec!["c1".into(), "c2".into()],
            start_ms: 1500,
            end_ms: 7250,
        };
        let ass = write_clip_ass(&p(), &clip, &Style::title_overlay(), 1080, 1920);
        assert!(ass.contains("Style: Default,"));
        assert!(ass.contains("Style: Title,"));
        assert!(ass.contains("PlayResX: 1080"));
        assert!(ass.contains("PlayResY: 1920"));
        // c1 (1500ms) becomes clip-relative 0; ends 3750-1500=2250ms = .25.
        assert!(ass.contains("Dialogue: 0,0:00:00.00,0:00:02.25,Default,,0,0,0,,Hello world"));
        // c2 starts 4000-1500=2500ms = 0:00:02.50.
        assert!(ass.contains("0:00:02.50"));
        // title overlay on layer 1 spans the whole 5750ms clip.
        assert!(ass.contains("Dialogue: 1,0:00:00.00,0:00:05.75,Title,,0,0,0,,Grace"));
    }

    #[test]
    fn clip_ass_excludes_out_of_range_captions() {
        use crate::model::Clip;
        let clip = Clip {
            id: "x".into(),
            title: "T".into(),
            hook: "".into(),
            caption_ids: vec!["c2".into()],
            start_ms: 4000,
            end_ms: 7250,
        };
        let ass = write_clip_ass(&p(), &clip, &Style::title_overlay(), 1080, 1920);
        assert!(!ass.contains("Hello world")); // c1 is before the clip
        assert!(ass.contains("This is two"));
    }

    // ── SRT ────────────────────────────────────────────────────────────────
    #[test]
    fn srt_basic_shape() {
        let out = write_srt(&p(), SrtOptions::default());
        assert!(out.starts_with("1\r\n00:00:01,500 --> 00:00:03,750\r\nHello world\r\n\r\n"));
        assert!(out.contains("2\r\n00:00:04,000 --> 00:00:07,250\r\nThis is two\r\n\r\n"));
    }

    // A caption whose text contains a blank line (e.g. a multi-line value pasted
    // via find/replace) must not break SRT/VTT cue framing. In SRT a blank line
    // ('\n\n') terminates a cue, so an embedded one truncates the cue and
    // desynchronises every following index — corrupting the whole file. The
    // writer must neutralise embedded blank lines (and bare CRs) so cue
    // boundaries stay intact.
    #[test]
    fn srt_caption_with_embedded_blank_line_does_not_break_cue_framing() {
        let mut proj = p();
        // One caption, single word, whose text holds a blank line.
        proj.captions = vec![Caption {
            id: "c".into(),
            start_ms: 0,
            end_ms: 1000,
            words: vec![Word::new("a\n\nb", 0, 1000, 90.0)],
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
            track_id: None,
        }];
        let out = write_srt(&proj, SrtOptions::default());
        // SRT parsers split cues on a blank line REGARDLESS of CR, so normalise
        // CRLF→LF and count blank-line separators: the only one must be the cue
        // terminator at the end. An embedded one splits this single cue in two
        // and desynchronises all later indices.
        let normalised = out.replace("\r\n", "\n");
        let blank_separators = normalised.matches("\n\n").count();
        assert_eq!(
            blank_separators, 1,
            "embedded blank line corrupted SRT cue framing: {out:?}"
        );
    }

    #[test]
    fn vtt_caption_with_embedded_blank_line_does_not_break_cue_framing() {
        let mut proj = p();
        proj.captions = vec![Caption {
            id: "c".into(),
            start_ms: 0,
            end_ms: 1000,
            words: vec![Word::new("a\n\nb", 0, 1000, 90.0)],
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
            track_id: None,
        }];
        let out = write_vtt(&proj, VttOptions::default());
        // After the "WEBVTT\n\n" header there must be exactly one cue, so exactly
        // one trailing blank-line separator. An embedded blank line would make
        // the cue text look like a second (timestamp-less) cue.
        let body = out.strip_prefix("WEBVTT\n\n").unwrap_or(&out);
        let blank_separators = body.matches("\n\n").count();
        assert_eq!(
            blank_separators, 1,
            "embedded blank line corrupted VTT cue framing: {out:?}"
        );
    }

    #[test]
    fn vtt_cue_ids_stay_contiguous_when_strip_empty_drops_a_cue() {
        // A wordless caption between two real ones must not leave a gap in the
        // VTT cue numbering (1, 2 — not 1, 3) when strip_empty removes it.
        let mut proj = p();
        proj.captions.insert(
            1,
            Caption {
                id: "empty".into(),
                start_ms: 3800,
                end_ms: 3900,
                words: vec![],
                speaker_id: None,
                style_id: None,
                notes: None,
                ai_generated: true,
                last_edited_at: 0,
                track_id: None,
            },
        );
        let out = write_vtt(
            &proj,
            VttOptions {
                include_speakers: false,
                strip_empty: true,
            },
        );
        let body = out.strip_prefix("WEBVTT\n\n").unwrap_or(&out);
        // Two cues emitted, numbered 1 then 2 (the dropped empty cue leaves no gap).
        assert!(
            body.starts_with("1\n"),
            "first cue id should be 1: {body:?}"
        );
        assert!(
            body.contains("\n2\n"),
            "second cue id should be 2: {body:?}"
        );
        assert!(
            !body.contains("\n3\n"),
            "no gap from the dropped cue: {body:?}"
        );
    }

    #[test]
    fn json_is_valid_and_preserves_words() {
        let out = write_json(&p(), JsonOptions::default());
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["format"], "sundayedit-captions");
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
    fn docx_is_a_valid_zip_with_required_parts_and_text() {
        use std::io::Read;
        let bytes = build_docx(&p(), TxtOptions::default()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("[Content_Types].xml").is_ok());
        assert!(zip.by_name("_rels/.rels").is_ok());
        let mut doc = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("Hello world"));
        assert!(doc.contains("This is two"));
        assert!(doc.contains("w:document"));
    }

    #[test]
    fn docx_escapes_xml_special_chars() {
        use std::io::Read;
        let mut proj = p();
        proj.captions = vec![Caption {
            id: "c".into(),
            start_ms: 0,
            end_ms: 1000,
            words: vec![
                Word::new("a", 0, 300, 90.0),
                Word::new("&", 300, 600, 90.0),
                Word::new("<b>", 600, 1000, 90.0),
            ],
            speaker_id: None,
            style_id: None,
            notes: None,
            ai_generated: true,
            last_edited_at: 0,
            track_id: None,
        }];
        let bytes = build_docx(&proj, TxtOptions::default()).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("&amp;"));
        assert!(doc.contains("&lt;b&gt;"));
        assert!(!doc.contains("<b>")); // raw tag must not leak into the body text
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
            track_id: None,
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

    // ── Property: timecode formatters are exactly reversible ─────────────────
    //
    // SRT/VTT timestamps are the load-bearing output: a player reads them back
    // by parsing HH:MM:SS,mmm / HH:MM:SS.mmm into milliseconds. The formatter is
    // only correct if a faithful parser recovers the exact same non-negative ms
    // (no rounding, truncation, or field-overflow drift). We parse the formatted
    // string back with an independent reference parser and assert equality across
    // a fixed adversarial table plus a fixed-seed PRNG sweep capped at 500.
    fn parse_hms_millis(s: &str, sep: char) -> i64 {
        // "HH:MM:SS<sep>mmm" — independent reference parser for the round-trip.
        let (hms, millis) = s.split_once(sep).expect("missing millis separator");
        let mut parts = hms.split(':');
        let h: i64 = parts.next().unwrap().parse().unwrap();
        let m: i64 = parts.next().unwrap().parse().unwrap();
        let sec: i64 = parts.next().unwrap().parse().unwrap();
        assert!(parts.next().is_none(), "too many ':' fields in {s:?}");
        assert!(m < 60 && sec < 60, "fields not normalised in {s:?}");
        assert_eq!(millis.len(), 3, "millis must be zero-padded to 3 in {s:?}");
        let ms: i64 = millis.parse().unwrap();
        ((h * 60 + m) * 60 + sec) * 1000 + ms
    }

    #[test]
    fn timecode_formatters_round_trip_to_exact_ms() {
        // Fixed adversarial table: boundaries that commonly break field math.
        let table: [i64; 12] = [
            0,
            1,
            999,
            1_000,
            59_999,
            60_000,
            3_599_999,
            3_600_000,
            3_661_001,
            86_399_999, // 23:59:59,999
            86_400_000, // 24:00:00,000 — hours overflow past two digits is fine
            359_999_999,
        ];
        for &ms in &table {
            assert_eq!(parse_hms_millis(&fmt_srt_time(ms), ','), ms, "srt ms={ms}");
            assert_eq!(parse_hms_millis(&fmt_vtt_time(ms), '.'), ms, "vtt ms={ms}");
        }

        // Fixed-seed PRNG sweep, ≤500 iterations, over the realistic 0..100h range.
        let mut state: u64 = 0xC0FF_EE12_3456_789B;
        let mut next = || {
            // xorshift64*
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        for _ in 0..500 {
            let ms = (next() % 360_000_000) as i64; // 0..100h
            assert_eq!(parse_hms_millis(&fmt_srt_time(ms), ','), ms, "srt ms={ms}");
            assert_eq!(parse_hms_millis(&fmt_vtt_time(ms), '.'), ms, "vtt ms={ms}");
        }
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

    #[test]
    fn hex_to_ass_bgr_falls_back_on_bad_input() {
        assert_eq!(hex_to_ass_bgr("#fff"), "&H00FFFFFF"); // too short
        assert_eq!(hex_to_ass_bgr("#zzzzzz"), "&H00FFFFFF"); // non-hex
                                                             // Multibyte char inside the first 6 bytes must NOT panic on a slice.
        assert_eq!(hex_to_ass_bgr("#café12"), "&H00FFFFFF");
    }

    #[test]
    fn ass_field_neutralizes_commas() {
        // A comma in a delimited field (speaker Name / Fontname) would shift every
        // following field — it must be neutralized; the Text field keeps commas.
        assert_eq!(ass_field("Smith, Jr."), "Smith  Jr.");
        assert_eq!(ass_field("a,b"), "a b");
        assert_eq!(ass_field("Arial"), "Arial");
        assert!(!ass_field("{x},y").contains(','));
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
