//! Sunday-link deep-link import — Phase 8.
//!
//! SundayEdit can be launched by a sister Sunday-suite app (SundayRec today,
//! SundayStage later) with a `sundayedit://import?…` URL so a recording flows
//! straight into captioning with its context already filled in. This module is
//! the pure, testable core: it turns the raw URL into a validated
//! `ImportRequest`. The native plumbing — OS scheme registration, app
//! lifecycle, emitting the parsed request to the renderer — lives in `lib.rs`;
//! the renderer drives the actual import using the existing project/context/
//! glossary flows.
//!
//! Contract (mirrors `docs/integration/*` in the SundayRec repo; we own the
//! `sundayedit://` scheme):
//!
//! ```text
//! sundayedit://import
//!   ?path=<absolute path to the source video, REQUIRED>
//!   &language=<ISO code, optional>            e.g. "no"
//!   &context=<free-text priming, optional>    e.g. "Sermon, speaker: Ola Nordmann"
//!   &glossary=<comma-separated terms>         e.g. "Ola Nordmann,kerygma"
//!   &returnTo=<caller scheme, optional>       e.g. "sundayrec"
//! ```
//!
//! Everything is percent-decoded (`+` is also treated as a space, per the
//! `application/x-www-form-urlencoded` convention). Unknown query keys are
//! ignored so the contract can grow without breaking older builds.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};

/// The scheme SundayEdit registers for inbound deep links.
pub const SCHEME: &str = "sundayedit";
/// The only action understood today.
pub const ACTION_IMPORT: &str = "import";

/// A validated request to import a video and seed its captioning context,
/// parsed from a `sundayedit://import?…` deep link. The renderer turns this
/// into a real project via the normal import + context/glossary flows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ImportRequest.ts")]
pub struct ImportRequest {
    /// Absolute path to the source video/audio file. Always present.
    pub path: String,
    /// ISO language code to transcribe in, if the caller specified one.
    pub language: Option<String>,
    /// Free-text priming for context-aware recognition (killer feature #2).
    pub context: Option<String>,
    /// Glossary terms to seed (speaker names, jargon) — de-duplicated,
    /// order preserved.
    pub glossary: Vec<String>,
    /// Scheme of the app that launched us, so we can hand results back
    /// (e.g. `"sundayrec"`). `None` for a plain user-initiated link.
    pub return_to: Option<String>,
}

/// Parse a `sundayedit://import?…` URL into an [`ImportRequest`].
///
/// Returns [`AppError::Validation`] for anything that isn't a well-formed
/// import link with a non-empty `path`.
pub fn parse_import_url(url: &str) -> AppResult<ImportRequest> {
    let trimmed = url.trim();

    // Strip the scheme (case-insensitive), tolerating `://` or a bare `:`.
    let rest = strip_scheme(trimmed, SCHEME)
        .ok_or_else(|| AppError::Validation(format!("not a {SCHEME}:// link: {url}")))?;

    // Split `action[?query]`. The action is everything up to the first `?`,
    // `/`, or `#`; a trailing slash (`import/?…`) is tolerated.
    let (action_part, query) = match rest.split_once('?') {
        Some((a, q)) => (a, q),
        None => (rest, ""),
    };
    let action = action_part.trim_end_matches('/').trim_start_matches('/');
    if !action.eq_ignore_ascii_case(ACTION_IMPORT) {
        return Err(AppError::Validation(format!(
            "unsupported deep-link action: {action:?} (expected {ACTION_IMPORT:?})"
        )));
    }

    let mut path: Option<String> = None;
    let mut language: Option<String> = None;
    let mut context: Option<String> = None;
    let mut glossary: Vec<String> = Vec::new();
    let mut return_to: Option<String> = None;

    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (raw_key, raw_val) = pair.split_once('=').unwrap_or((pair, ""));
        let key = decode_component(raw_key);
        let value = decode_component(raw_val);
        match key.as_str() {
            "path" => path = non_empty(value),
            "language" | "lang" => language = non_empty(value),
            "context" => context = non_empty(value),
            "glossary" => glossary = split_glossary(&value),
            "returnTo" | "return_to" => return_to = non_empty(value),
            _ => {} // forward-compatible: ignore unknown keys
        }
    }

    let path = path.ok_or_else(|| {
        AppError::Validation("deep-link import is missing a non-empty `path`".into())
    })?;

    Ok(ImportRequest {
        path,
        language,
        context,
        glossary,
        return_to,
    })
}

/// If `s` begins with `scheme:` (case-insensitive), return the remainder with
/// any leading `//` removed. Otherwise `None`.
fn strip_scheme<'a>(s: &'a str, scheme: &str) -> Option<&'a str> {
    let prefix_len = scheme.len() + 1; // "+ 1" for the ':'
    if s.len() < prefix_len {
        return None;
    }
    let (head, tail) = s.split_at(prefix_len);
    let (name, colon) = head.split_at(scheme.len());
    if colon != ":" || !name.eq_ignore_ascii_case(scheme) {
        return None;
    }
    Some(tail.strip_prefix("//").unwrap_or(tail))
}

/// `Some(trimmed)` if non-empty after trimming, else `None`.
fn non_empty(s: String) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Split a comma-separated glossary value into trimmed, non-empty, de-duplicated
/// terms (case-insensitive dedupe, first spelling wins, original order kept).
fn split_glossary(value: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for term in value.split(',') {
        let t = term.trim();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_lowercase();
        if seen.contains(&lower) {
            continue;
        }
        seen.push(lower);
        out.push(t.to_string());
    }
    out
}

/// Build the hand-back URL the deep-link caller listens for, once captions have
/// been written to a sidecar next to the source video. It points the caller's
/// own scheme at that file: `sundayrec://captions?path=<encoded sidecar>`.
///
/// `return_to` must be a clean URL scheme (RFC 3986: ALPHA followed by
/// alphanumerics / `+` / `-` / `.`); the path is percent-encoded.
pub fn captions_callback_url(return_to: &str, sidecar_path: &str) -> AppResult<String> {
    let scheme = return_to.trim();
    let valid_scheme = !scheme.is_empty()
        && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme {
        return Err(AppError::Validation(format!(
            "invalid returnTo scheme: {return_to:?}"
        )));
    }
    Ok(format!(
        "{scheme}://captions?path={}",
        encode_component(sidecar_path)
    ))
}

/// Percent-encode a string as a URL query-component value: RFC 3986 unreserved
/// characters (`A-Z a-z 0-9 - _ . ~`) pass through, everything else (including
/// `/`, spaces and non-ASCII) becomes `%XX`. The exact inverse of
/// [`decode_component`] for any input (spaces round-trip via `%20`, never `+`).
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(hex_digit(b >> 4));
                out.push(hex_digit(b & 0x0f));
            }
        }
    }
    out
}

fn hex_digit(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'A' + (n - 10)) as char,
    }
}

/// Percent-decode one query component. `%XX` → byte, `+` → space, everything
/// else left as-is. Invalid `%` escapes are left unchanged rather than rejected, so a
/// stray `%` in a path never sinks the whole import. Bytes are reassembled and
/// read as UTF-8 (lossily) at the end.
fn decode_component(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                match (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    (Some(hi), Some(lo)) => {
                        out.push(hi << 4 | lo);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_full_link() {
        let url = "sundayedit://import?path=%2FUsers%2Fola%2Fclip.mp4&language=no\
                   &context=Sermon%2C+speaker%3A+Ola+Nordmann\
                   &glossary=Ola+Nordmann%2Ckerygma%2Csoteriology&returnTo=sundayrec";
        let req = parse_import_url(url).unwrap();
        assert_eq!(req.path, "/Users/ola/clip.mp4");
        assert_eq!(req.language.as_deref(), Some("no"));
        assert_eq!(req.context.as_deref(), Some("Sermon, speaker: Ola Nordmann"));
        assert_eq!(req.glossary, vec!["Ola Nordmann", "kerygma", "soteriology"]);
        assert_eq!(req.return_to.as_deref(), Some("sundayrec"));
    }

    #[test]
    fn path_is_required() {
        let err = parse_import_url("sundayedit://import?language=no").unwrap_err();
        assert_eq!(err.code(), "validation");
        // An empty/whitespace path counts as missing.
        assert!(parse_import_url("sundayedit://import?path=%20%20").is_err());
    }

    #[test]
    fn rejects_wrong_scheme_and_action() {
        assert!(parse_import_url("https://import?path=/a.mp4").is_err());
        assert!(parse_import_url("sundayrec://import?path=/a.mp4").is_err());
        assert!(parse_import_url("sundayedit://export?path=/a.mp4").is_err());
    }

    #[test]
    fn scheme_and_action_are_case_insensitive() {
        let req = parse_import_url("SundayEdit://Import?path=/a.mp4").unwrap();
        assert_eq!(req.path, "/a.mp4");
    }

    #[test]
    fn tolerates_trailing_slash_after_action() {
        let req = parse_import_url("sundayedit://import/?path=/a.mp4").unwrap();
        assert_eq!(req.path, "/a.mp4");
    }

    #[test]
    fn optional_fields_default_cleanly() {
        let req = parse_import_url("sundayedit://import?path=/a.mp4").unwrap();
        assert_eq!(req.language, None);
        assert_eq!(req.context, None);
        assert!(req.glossary.is_empty());
        assert_eq!(req.return_to, None);
    }

    #[test]
    fn glossary_trims_drops_empties_and_dedupes() {
        let req =
            parse_import_url("sundayedit://import?path=/a.mp4&glossary=+Ada+,,ada,+,Babbage")
                .unwrap();
        // "Ada" kept (first spelling), case-insensitive "ada" dropped, blanks gone.
        assert_eq!(req.glossary, vec!["Ada", "Babbage"]);
    }

    #[test]
    fn accepts_lang_and_return_to_aliases() {
        let req =
            parse_import_url("sundayedit://import?path=/a.mp4&lang=de&return_to=sundaystage")
                .unwrap();
        assert_eq!(req.language.as_deref(), Some("de"));
        assert_eq!(req.return_to.as_deref(), Some("sundaystage"));
    }

    #[test]
    fn ignores_unknown_keys() {
        let req = parse_import_url("sundayedit://import?path=/a.mp4&futureFlag=1").unwrap();
        assert_eq!(req.path, "/a.mp4");
    }

    #[test]
    fn lone_percent_is_left_intact() {
        // A stray '%' (not a valid escape) must not lose the rest of the path.
        let req = parse_import_url("sundayedit://import?path=/a%b/c.mp4").unwrap();
        assert_eq!(req.path, "/a%b/c.mp4");
    }

    #[test]
    fn encode_decode_round_trips() {
        for s in [
            "/Users/ola/My Talk (2026).srt",
            "C:\\Users\\Ola\\tale.vtt",
            "kerygma + søndag/æøå",
            "",
        ] {
            assert_eq!(decode_component(&encode_component(s)), s, "round-trip {s:?}");
        }
        // Spaces encode as %20 (not +), so they survive the +→space decode rule.
        assert_eq!(encode_component("a b"), "a%20b");
    }

    #[test]
    fn builds_a_callback_url() {
        let url = captions_callback_url("sundayrec", "/Users/ola/a b.srt").unwrap();
        assert_eq!(url, "sundayrec://captions?path=%2FUsers%2Fola%2Fa%20b.srt");
        // The caller can parse it straight back to the path.
        assert_eq!(decode_component("%2FUsers%2Fola%2Fa%20b.srt"), "/Users/ola/a b.srt");
    }

    #[test]
    fn rejects_a_bad_return_to_scheme() {
        assert!(captions_callback_url("", "/a.srt").is_err());
        assert!(captions_callback_url("ht tp", "/a.srt").is_err());
        assert!(captions_callback_url("1bad", "/a.srt").is_err()); // must start with a letter
        assert!(captions_callback_url("a/b", "/a.srt").is_err());
    }
}
