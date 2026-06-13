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
//! ## Canonical contract
//!
//! The deep-link **grammar** now lives in the shared `sunday-contracts` crate as
//! [`MediaHandoff`] — the suite-wide superset of what was originally
//! SundayEdit's own format. We delegate all URL parsing to
//! [`sunday_contracts::deeplink::parse_handoff_url`] so there is a single source
//! of truth for the wire format across Rec → Edit and Edit → other apps. This
//! module is a thin app-facing adapter:
//!
//! * [`ImportRequest`] is the renderer-facing, `ts-rs`-exported view of a
//!   handoff. It carries the **full** canonical field set (path, media kind,
//!   language, context, glossary, service/church ids, returnTo) so nothing is
//!   silently dropped, while keeping the generated TypeScript binding the
//!   renderer already consumes. The canonical `MediaHandoff` itself has no
//!   `ts-rs` derive, so a literal type-import would lose the binding — hence the
//!   conversion layer below plus a round-trip parity test (`canonical_*`) that
//!   fails if the two field sets ever drift apart.
//! * [`captions_callback_url`] is app-specific (it echoes the original
//!   `recording` path so SundayRec can attach captions to the right file) and
//!   has no equivalent in the canonical crate, so it stays here.
//!
//! Contract (handled by `sunday-contracts`):
//!
//! ```text
//! sundayedit://import
//!   ?path=<absolute path to the source media, REQUIRED>
//!   &media_kind=<video|audio, optional>
//!   &language=<ISO code, optional>            e.g. "no"
//!   &context=<free-text priming, optional>    e.g. "Sermon, speaker: Ola Nordmann"
//!   &glossary=<comma-separated terms>         e.g. "Ola Nordmann,kerygma"
//!   &service_id=<id, optional>                e.g. a SundayPlan service
//!   &church_id=<id, optional>
//!   &returnTo=<caller scheme, optional>       e.g. "sundayrec"
//! ```
//!
//! Everything is percent-decoded (`+` is also treated as a space, per the
//! `application/x-www-form-urlencoded` convention). Unknown query keys are
//! ignored so the contract can grow without breaking older builds.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use sunday_contracts as canonical;
use sunday_contracts::MediaHandoff;

use crate::error::{AppError, AppResult};
use crate::services::video::MediaKind;

/// The scheme SundayEdit registers for inbound deep links.
pub const SCHEME: &str = "sundayedit";
/// The only action understood today. Re-exported from the canonical crate so
/// the value is shared suite-wide.
pub const ACTION_IMPORT: &str = canonical::ACTION_IMPORT;

/// A validated request to import a media file and seed its captioning context,
/// parsed from a `sundayedit://import?…` deep link. The renderer turns this
/// into a real project via the normal import + context/glossary flows.
///
/// This is the app-facing, `ts-rs`-exported projection of the canonical
/// [`MediaHandoff`]; it carries the full superset of handoff fields so Rec → Edit
/// (and future Edit → other) handoffs lose nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/ImportRequest.ts")]
pub struct ImportRequest {
    /// Absolute path to the source video/audio file. Always present.
    pub path: String,
    /// Whether the caller flagged this as video or audio-only, if specified.
    /// Maps the canonical `video`/`audio` wire kinds onto SundayEdit's own
    /// `Video`/`AudioOnly` media classification.
    pub media_kind: Option<MediaKind>,
    /// ISO language code to transcribe in, if the caller specified one.
    pub language: Option<String>,
    /// Free-text priming for context-aware recognition (killer feature #2).
    pub context: Option<String>,
    /// Glossary terms to seed (speaker names, jargon) — de-duplicated,
    /// order preserved.
    pub glossary: Vec<String>,
    /// SundayPlan service the media belongs to, if the caller knows it.
    pub service_id: Option<String>,
    /// Church / tenant id the media belongs to, if the caller knows it.
    pub church_id: Option<String>,
    /// Scheme of the app that launched us, so we can hand results back
    /// (e.g. `"sundayrec"`). `None` for a plain user-initiated link.
    pub return_to: Option<String>,
}

impl From<MediaHandoff> for ImportRequest {
    fn from(h: MediaHandoff) -> Self {
        ImportRequest {
            path: h.path,
            media_kind: h.media_kind.map(MediaKind::from_wire),
            language: h.language,
            context: h.context,
            glossary: h.glossary,
            service_id: h.service_id,
            church_id: h.church_id,
            return_to: h.return_to,
        }
    }
}

impl From<&ImportRequest> for MediaHandoff {
    fn from(r: &ImportRequest) -> Self {
        MediaHandoff {
            action: ACTION_IMPORT.to_string(),
            path: r.path.clone(),
            media_kind: r.media_kind.map(MediaKind::to_wire),
            language: r.language.clone(),
            context: r.context.clone(),
            glossary: r.glossary.clone(),
            service_id: r.service_id.clone(),
            church_id: r.church_id.clone(),
            return_to: r.return_to.clone(),
        }
    }
}

impl MediaKind {
    /// Map the canonical wire `MediaKind` onto SundayEdit's own classification
    /// (the wire contract has no "audio_only" — its `Audio` is our `AudioOnly`).
    fn from_wire(k: canonical::MediaKind) -> Self {
        match k {
            canonical::MediaKind::Video => MediaKind::Video,
            canonical::MediaKind::Audio => MediaKind::AudioOnly,
        }
    }

    /// Inverse of [`MediaKind::from_wire`].
    fn to_wire(self) -> canonical::MediaKind {
        match self {
            MediaKind::Video => canonical::MediaKind::Video,
            MediaKind::AudioOnly => canonical::MediaKind::Audio,
        }
    }
}

/// Parse a `sundayedit://import?…` URL into an [`ImportRequest`].
///
/// Delegates the grammar to the shared `sunday-contracts` crate (single source
/// of truth across the suite) and adapts the result into the app-facing,
/// `ts-rs`-exported shape, mapping the canonical error into [`AppError`].
///
/// Returns [`AppError::Validation`] for anything that isn't a well-formed
/// import link with a non-empty `path`.
pub fn parse_import_url(url: &str) -> AppResult<ImportRequest> {
    canonical::parse_handoff_url(url, SCHEME)
        .map(ImportRequest::from)
        .map_err(|e| AppError::Validation(e.0))
}

/// Build the hand-back URL the deep-link caller listens for, once captions have
/// been written to a sidecar next to the source video. It points the caller's
/// own scheme at that file: `sundayrec://captions?path=<encoded sidecar>`.
///
/// When `recording_path` is supplied (the original media the caller sent us via
/// `sundayedit://import`), it's echoed back as `&recording=<encoded>` so the
/// caller can attach the captions to the right recording without guessing —
/// e.g. SundayRec writes `<recording>.transcript.json`. It's optional: a plain
/// user-saved SRT (no inbound deep link) has no recording to echo.
///
/// This is SundayEdit-specific (the canonical crate's `result_callback_url` has
/// no recording echo and uses a `result` action), so it lives here; it reuses
/// the canonical percent-codec to stay byte-for-byte compatible.
///
/// `return_to` must be a clean URL scheme (RFC 3986: ALPHA followed by
/// alphanumerics / `+` / `-` / `.`); both paths are percent-encoded.
pub fn captions_callback_url(
    return_to: &str,
    sidecar_path: &str,
    recording_path: Option<&str>,
) -> AppResult<String> {
    let scheme = return_to.trim();
    let valid_scheme = !scheme.is_empty()
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !valid_scheme {
        return Err(AppError::Validation(format!(
            "invalid returnTo scheme: {return_to:?}"
        )));
    }
    let mut url = format!(
        "{scheme}://captions?path={}",
        encode_component(sidecar_path)
    );
    if let Some(rec) = recording_path.map(str::trim).filter(|s| !s.is_empty()) {
        url.push_str("&recording=");
        url.push_str(&encode_component(rec));
    }
    Ok(url)
}

/// Percent-encode a string as a URL query-component value: RFC 3986 unreserved
/// characters (`A-Z a-z 0-9 - _ . ~`) pass through, everything else (including
/// `/`, spaces and non-ASCII) becomes `%XX`. Spaces round-trip via `%20`, never
/// `+`. Re-exported from the canonical crate so every Sunday app encodes
/// identically.
pub use canonical::encode_component;

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
        assert_eq!(
            req.context.as_deref(),
            Some("Sermon, speaker: Ola Nordmann")
        );
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
        assert_eq!(req.media_kind, None);
        assert_eq!(req.language, None);
        assert_eq!(req.context, None);
        assert!(req.glossary.is_empty());
        assert_eq!(req.service_id, None);
        assert_eq!(req.church_id, None);
        assert_eq!(req.return_to, None);
    }

    #[test]
    fn carries_the_full_superset() {
        let req = parse_import_url(
            "sundayedit://import?path=/a.mp4&media_kind=audio\
             &service_id=svc-1&church_id=ch-1",
        )
        .unwrap();
        assert_eq!(req.media_kind, Some(MediaKind::AudioOnly));
        assert_eq!(req.service_id.as_deref(), Some("svc-1"));
        assert_eq!(req.church_id.as_deref(), Some("ch-1"));
        // `media_kind=video` maps onto the app's own Video classification.
        let req = parse_import_url("sundayedit://import?path=/a.mp4&media_kind=video").unwrap();
        assert_eq!(req.media_kind, Some(MediaKind::Video));
    }

    #[test]
    fn glossary_trims_drops_empties_and_dedupes() {
        let req = parse_import_url("sundayedit://import?path=/a.mp4&glossary=+Ada+,,ada,+,Babbage")
            .unwrap();
        // "Ada" kept (first spelling), case-insensitive "ada" dropped, blanks gone.
        assert_eq!(req.glossary, vec!["Ada", "Babbage"]);
    }

    #[test]
    fn accepts_lang_and_return_to_aliases() {
        let req = parse_import_url("sundayedit://import?path=/a.mp4&lang=de&return_to=sundaystage")
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
    fn encode_round_trips_via_percent20() {
        // Spaces encode as %20 (not +), so they survive the +→space decode rule.
        assert_eq!(encode_component("a b"), "a%20b");
        assert_eq!(
            encode_component("/Users/ola/My Talk.srt"),
            "%2FUsers%2Fola%2FMy%20Talk.srt"
        );
    }

    #[test]
    fn builds_a_callback_url() {
        let url = captions_callback_url("sundayrec", "/Users/ola/a b.srt", None).unwrap();
        assert_eq!(url, "sundayrec://captions?path=%2FUsers%2Fola%2Fa%20b.srt");
    }

    #[test]
    fn callback_url_echoes_the_recording_path() {
        // The recording the caller sent us is echoed back so it can attach the
        // captions without guessing (SundayRec writes <recording>.transcript.json).
        let url = captions_callback_url(
            "sundayrec",
            "/Users/ola/tale.srt",
            Some("/Users/ola/tale.mp4"),
        )
        .unwrap();
        assert_eq!(
            url,
            "sundayrec://captions?path=%2FUsers%2Fola%2Ftale.srt&recording=%2FUsers%2Fola%2Ftale.mp4"
        );
        // A blank/whitespace recording is treated as absent (no empty param).
        let url = captions_callback_url("sundayrec", "/a.srt", Some("  ")).unwrap();
        assert_eq!(url, "sundayrec://captions?path=%2Fa.srt");
    }

    #[test]
    fn rejects_a_bad_return_to_scheme() {
        assert!(captions_callback_url("", "/a.srt", None).is_err());
        assert!(captions_callback_url("ht tp", "/a.srt", None).is_err());
        assert!(captions_callback_url("1bad", "/a.srt", None).is_err()); // must start with a letter
        assert!(captions_callback_url("a/b", "/a.srt", None).is_err());
    }

    // ── Canonical convergence: kill silent drift ────────────────────────────
    //
    // `ImportRequest` is a hand-written projection of the canonical
    // `MediaHandoff`. If the canonical type grows or renames a field, these
    // tests fail so we notice instead of silently dropping data on the wire.

    #[test]
    fn canonical_round_trips_the_full_superset() {
        // Build a fully-populated canonical handoff, render it via the canonical
        // builder, parse it through our adapter, and convert back. Equality
        // proves every canonical field survives the SundayEdit round-trip.
        let original = MediaHandoff {
            action: ACTION_IMPORT.to_string(),
            path: "/Users/ola/My Talk (2026).mov".to_string(),
            media_kind: Some(canonical::MediaKind::Video),
            language: Some("no".to_string()),
            context: Some("Sermon, speaker: Ola".to_string()),
            glossary: vec!["Ola".to_string(), "kerygma".to_string()],
            service_id: Some("svc-1".to_string()),
            church_id: Some("ch-1".to_string()),
            return_to: Some("sundayrec".to_string()),
        };
        let url = canonical::build_handoff_url(SCHEME, &original);
        let req = parse_import_url(&url).unwrap();
        let round_tripped: MediaHandoff = (&req).into();
        assert_eq!(
            round_tripped, original,
            "a canonical field drifted in the SundayEdit adapter"
        );
    }

    #[test]
    fn canonical_audio_kind_maps_to_audio_only() {
        // The wire `audio` kind must survive as the app's `AudioOnly`, and back.
        let original = MediaHandoff {
            action: ACTION_IMPORT.to_string(),
            path: "/a.mp3".to_string(),
            media_kind: Some(canonical::MediaKind::Audio),
            language: None,
            context: None,
            glossary: vec![],
            service_id: None,
            church_id: None,
            return_to: None,
        };
        let url = canonical::build_handoff_url(SCHEME, &original);
        let req = parse_import_url(&url).unwrap();
        assert_eq!(req.media_kind, Some(MediaKind::AudioOnly));
        assert_eq!(MediaHandoff::from(&req), original);
    }
}
