//! Cloud ASR providers — Phase 2.2.
//!
//! Cloud is OFF by default. The first time a user enables it, the UI must
//! show a consent dialog ("Your video's audio will be uploaded to X").
//! API keys live in the OS keychain (`keyring` crate, wired in the
//! command layer), never plaintext.
//!
//! All three providers (OpenAI Whisper, AssemblyAI, Deepgram) are wired for
//! live transcription (BYOK). The correctness-critical part — parsing each
//! provider's response JSON into our normalized `Transcript`, with confidence
//! run through the SAME curve as local Whisper so tiers line up across
//! backends — is pure and unit-tested below.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::services::asr::confidence::{
    provider_confidence_to_scale, word_confidence_from_token_logprobs,
};
use crate::services::asr::{Segment, TranscribedWord, Transcript};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../src/lib/bindings/CloudProvider.ts")]
pub enum CloudProvider {
    OpenaiWhisper,
    AssemblyAi,
    Deepgram,
}

impl CloudProvider {
    pub fn display_name(&self) -> &'static str {
        match self {
            CloudProvider::OpenaiWhisper => "OpenAI Whisper",
            CloudProvider::AssemblyAi => "AssemblyAI",
            CloudProvider::Deepgram => "Deepgram",
        }
    }

    /// Privacy notice shown in the consent dialog before first use.
    pub fn consent_text(&self) -> String {
        format!(
            "Your video's audio will be uploaded to {} for transcription. \
             Review their privacy policy before continuing.",
            self.display_name()
        )
    }

    /// Approximate list price in USD per audio minute (pay-as-you-go, 2026).
    /// Shown as a *ballpark* — providers change pricing, so the UI labels it
    /// "estimated".
    pub fn price_per_min_usd(&self) -> f64 {
        match self {
            CloudProvider::OpenaiWhisper => 0.006,
            CloudProvider::AssemblyAi => 0.0062,
            CloudProvider::Deepgram => 0.0043,
        }
    }

    pub fn privacy_url(&self) -> &'static str {
        match self {
            CloudProvider::OpenaiWhisper => "https://openai.com/policies/privacy-policy",
            CloudProvider::AssemblyAi => "https://www.assemblyai.com/legal/privacy-policy",
            CloudProvider::Deepgram => "https://deepgram.com/privacy",
        }
    }

    /// Whether the provider returns real per-word confidence (drives killer
    /// feature #1). OpenAI's API only exposes segment-level avg_logprob, so we
    /// can only *approximate* per-word confidence from it.
    pub fn has_word_confidence(&self) -> bool {
        match self {
            CloudProvider::OpenaiWhisper => false,
            CloudProvider::AssemblyAi | CloudProvider::Deepgram => true,
        }
    }

    pub fn all() -> [CloudProvider; 3] {
        [
            CloudProvider::OpenaiWhisper,
            CloudProvider::AssemblyAi,
            CloudProvider::Deepgram,
        ]
    }

    pub fn info(&self) -> CloudProviderInfo {
        CloudProviderInfo {
            provider: *self,
            display_name: self.display_name().to_string(),
            price_per_min_usd: self.price_per_min_usd(),
            privacy_url: self.privacy_url().to_string(),
            word_confidence: self.has_word_confidence(),
            consent_text: self.consent_text(),
        }
    }
}

/// Catalog row for the cloud-provider picker.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/CloudProviderInfo.ts")]
pub struct CloudProviderInfo {
    pub provider: CloudProvider,
    pub display_name: String,
    pub price_per_min_usd: f64,
    pub privacy_url: String,
    pub word_confidence: bool,
    pub consent_text: String,
}

/// A pre-submit cost preview, per the plan's cost-transparency requirement.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/CloudCostEstimate.ts")]
pub struct CloudCostEstimate {
    pub provider: CloudProvider,
    pub minutes: f64,
    pub estimated_usd: f64,
}

pub fn catalog() -> Vec<CloudProviderInfo> {
    CloudProvider::all().iter().map(|p| p.info()).collect()
}

/// Estimate the cost of transcribing `duration_ms` of audio with `provider`.
pub fn estimate_cost(provider: CloudProvider, duration_ms: i64) -> CloudCostEstimate {
    let minutes = (duration_ms.max(0) as f64) / 60_000.0;
    CloudCostEstimate {
        provider,
        minutes,
        estimated_usd: minutes * provider.price_per_min_usd(),
    }
}

fn sec_to_ms(sec: f64) -> i64 {
    (sec * 1000.0).round() as i64
}

// ── OpenAI Whisper API (verbose_json) ─────────────────────────────────────────
//
// The /audio/transcriptions endpoint with response_format=verbose_json
// returns segments with `avg_logprob`. It does NOT return per-word
// confidence directly, but the verbose response includes word timings
// (timestamp_granularities[]=word) — we approximate per-word confidence
// from the segment's avg_logprob (the best signal available) until OpenAI
// exposes per-word logprobs.

pub fn parse_openai_verbose_json(json: &str) -> AppResult<Transcript> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let language = v
        .get("language")
        .and_then(|l| l.as_str())
        .unwrap_or("auto")
        .to_string();

    let segments = v
        .get("segments")
        .and_then(|s| s.as_array())
        .ok_or_else(|| AppError::Validation("OpenAI response has no segments".into()))?;

    let mut out_segments = Vec::new();
    for seg in segments {
        let seg_start = seg.get("start").and_then(|s| s.as_f64()).unwrap_or(0.0);
        let seg_end = seg.get("end").and_then(|e| e.as_f64()).unwrap_or(seg_start);
        let avg_logprob = seg
            .get("avg_logprob")
            .and_then(|l| l.as_f64())
            .unwrap_or(-0.2) as f32;
        let seg_conf = word_confidence_from_token_logprobs(&[avg_logprob]);

        // Word-level timings when present (timestamp_granularities=word).
        let words: Vec<TranscribedWord> =
            if let Some(ws) = seg.get("words").and_then(|w| w.as_array()) {
                ws.iter()
                    .filter_map(|w| {
                        let text = w.get("word").and_then(|t| t.as_str())?.trim().to_string();
                        if text.is_empty() {
                            return None;
                        }
                        let start = w.get("start").and_then(|s| s.as_f64()).unwrap_or(seg_start);
                        let end = w.get("end").and_then(|e| e.as_f64()).unwrap_or(seg_end);
                        Some(TranscribedWord {
                            text,
                            start_ms: sec_to_ms(start),
                            end_ms: sec_to_ms(end),
                            confidence: seg_conf, // segment-level estimate
                        })
                    })
                    .collect()
            } else {
                // No word timings — fall back to the whole segment text as one "word".
                let text = seg
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![TranscribedWord {
                        text,
                        start_ms: sec_to_ms(seg_start),
                        end_ms: sec_to_ms(seg_end),
                        confidence: seg_conf,
                    }]
                }
            };

        if !words.is_empty() {
            out_segments.push(Segment {
                start_ms: sec_to_ms(seg_start),
                end_ms: sec_to_ms(seg_end),
                words,
            });
        }
    }

    Ok(Transcript {
        language,
        segments: out_segments,
        backend: "OpenAI Whisper".into(),
    })
}

// ── AssemblyAI ────────────────────────────────────────────────────────────────
//
// Returns word-level confidence in 0..1 directly, with start/end in ms.

pub fn parse_assemblyai_json(json: &str) -> AppResult<Transcript> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let language = v
        .get("language_code")
        .and_then(|l| l.as_str())
        .unwrap_or("auto")
        .to_string();

    let words_arr = v
        .get("words")
        .and_then(|w| w.as_array())
        .ok_or_else(|| AppError::Validation("AssemblyAI response has no words".into()))?;

    let words: Vec<TranscribedWord> = words_arr
        .iter()
        .filter_map(|w| {
            let text = w.get("text").and_then(|t| t.as_str())?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            let start = w.get("start").and_then(|s| s.as_i64()).unwrap_or(0);
            let end = w.get("end").and_then(|e| e.as_i64()).unwrap_or(start);
            let raw_conf = w.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0) as f32;
            Some(TranscribedWord {
                text,
                start_ms: start,
                end_ms: end,
                confidence: provider_confidence_to_scale(raw_conf),
            })
        })
        .collect();

    // AssemblyAI is a flat word list; wrap as one segment.
    let segments = if words.is_empty() {
        Vec::new()
    } else {
        vec![Segment {
            start_ms: words.first().unwrap().start_ms,
            end_ms: words.last().unwrap().end_ms,
            words,
        }]
    };

    Ok(Transcript {
        language,
        segments,
        backend: "AssemblyAI".into(),
    })
}

// ── Deepgram ──────────────────────────────────────────────────────────────────
//
// results.channels[0].alternatives[0].words[] with confidence 0..1 and
// start/end in SECONDS.

pub fn parse_deepgram_json(json: &str) -> AppResult<Transcript> {
    let v: serde_json::Value = serde_json::from_str(json)?;

    let alt = v
        .pointer("/results/channels/0/alternatives/0")
        .ok_or_else(|| AppError::Validation("Deepgram response missing alternatives".into()))?;

    let language = v
        .pointer("/results/channels/0/detected_language")
        .and_then(|l| l.as_str())
        .unwrap_or("auto")
        .to_string();

    let words_arr = alt
        .get("words")
        .and_then(|w| w.as_array())
        .ok_or_else(|| AppError::Validation("Deepgram alternative has no words".into()))?;

    let words: Vec<TranscribedWord> = words_arr
        .iter()
        .filter_map(|w| {
            // Prefer punctuated_word when present (better caption text).
            let text = w
                .get("punctuated_word")
                .or_else(|| w.get("word"))
                .and_then(|t| t.as_str())?
                .trim()
                .to_string();
            if text.is_empty() {
                return None;
            }
            let start = w.get("start").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let end = w.get("end").and_then(|e| e.as_f64()).unwrap_or(start);
            let raw_conf = w.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.0) as f32;
            Some(TranscribedWord {
                text,
                start_ms: sec_to_ms(start),
                end_ms: sec_to_ms(end),
                confidence: provider_confidence_to_scale(raw_conf),
            })
        })
        .collect();

    let segments = if words.is_empty() {
        Vec::new()
    } else {
        vec![Segment {
            start_ms: words.first().unwrap().start_ms,
            end_ms: words.last().unwrap().end_ms,
            words,
        }]
    };

    Ok(Transcript {
        language,
        segments,
        backend: "Deepgram".into(),
    })
}

// ── Live transcription calls ──────────────────────────────────────────────
//
// All three providers are wired. They use different shapes:
//   - OpenAI Whisper: one synchronous multipart POST.
//   - Deepgram: one synchronous POST of the raw audio bytes.
//   - AssemblyAI: upload the bytes, request a transcript, then poll until done.
// BYOK: the key is resolved from the OS keychain in the command layer and
// passed in. Response parsing is the pure, tested part above.

/// Pull a human message out of an OpenAI error body, else the raw text.
fn openai_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect())
}

/// Best-effort human message from a provider error body across the common
/// shapes ({"error": "..."}, {"error": {"message": "..."}}, {"err_msg": ...}).
fn provider_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(s) = v["error"]["message"].as_str() {
            return s.to_string();
        }
        for key in ["error", "err_msg", "message"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                return s.to_string();
            }
        }
    }
    body.chars().take(300).collect()
}

/// Content-Type for a media file, inferred from its extension. Deepgram sniffs
/// container formats regardless, but a correct type avoids ambiguity.
fn mime_for(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("m4a") | Some("aac") => "audio/mp4",
        Some("flac") => "audio/flac",
        Some("ogg") | Some("opus") => "audio/ogg",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

/// Transcribe an audio/video file via the OpenAI Whisper API (whisper-1,
/// `verbose_json` + word timestamps) → our normalized `Transcript`.
pub async fn transcribe_openai(
    audio_path: &std::path::Path,
    api_key: &str,
    language: &str,
) -> AppResult<Transcript> {
    if api_key.trim().is_empty() {
        return Err(AppError::Validation(
            "OpenAI API key is not set — add it under Settings → API keys.".into(),
        ));
    }

    let bytes = tokio::fs::read(audio_path).await?;
    let filename = audio_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("audio")
        .to_string();

    let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
    let mut form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "word")
        .part("file", part);
    if !language.is_empty() && language != "auto" {
        form = form.text("language", language.to_string());
    }

    let resp = reqwest::Client::new()
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "OpenAI transcription failed ({}): {}",
            status.as_u16(),
            openai_error_message(&body)
        )));
    }
    parse_openai_verbose_json(&body)
}

const ASSEMBLYAI_BASE: &str = "https://api.assemblyai.com/v2";
/// Poll cap for AssemblyAI: 400 × 3 s ≈ 20 minutes before we give up.
const ASSEMBLYAI_MAX_POLLS: u32 = 400;

/// Transcribe via AssemblyAI: upload the audio, request a transcript, then poll
/// until it's `completed` (or `error`). Returns our normalized `Transcript`.
pub async fn transcribe_assemblyai(
    audio_path: &std::path::Path,
    api_key: &str,
    language: &str,
) -> AppResult<Transcript> {
    if api_key.trim().is_empty() {
        return Err(AppError::Validation(
            "AssemblyAI API key is not set — add it under Settings → API keys.".into(),
        ));
    }

    let bytes = tokio::fs::read(audio_path).await?;
    let client = reqwest::Client::new();
    let net = |e: reqwest::Error| AppError::Network(e.to_string());

    // 1. Upload the raw audio → an upload_url AssemblyAI can read back.
    let resp = client
        .post(format!("{ASSEMBLYAI_BASE}/upload"))
        .header("authorization", api_key)
        .body(bytes)
        .send()
        .await
        .map_err(net)?;
    let status = resp.status();
    let body = resp.text().await.map_err(net)?;
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "AssemblyAI upload failed ({}): {}",
            status.as_u16(),
            provider_error_message(&body)
        )));
    }
    let upload_url = serde_json::from_str::<serde_json::Value>(&body)?
        .get("upload_url")
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::Validation("AssemblyAI upload returned no upload_url".into()))?;

    // 2. Request a transcript for that upload.
    let mut req_body = serde_json::json!({ "audio_url": upload_url });
    if language.is_empty() || language == "auto" {
        req_body["language_detection"] = serde_json::json!(true);
    } else {
        req_body["language_code"] = serde_json::json!(language);
    }
    let resp = client
        .post(format!("{ASSEMBLYAI_BASE}/transcript"))
        .header("authorization", api_key)
        .json(&req_body)
        .send()
        .await
        .map_err(net)?;
    let status = resp.status();
    let body = resp.text().await.map_err(net)?;
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "AssemblyAI transcript request failed ({}): {}",
            status.as_u16(),
            provider_error_message(&body)
        )));
    }
    let id = serde_json::from_str::<serde_json::Value>(&body)?
        .get("id")
        .and_then(|i| i.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::Validation("AssemblyAI transcript request returned no id".into())
        })?;

    // 3. Poll until completed / error.
    let poll_url = format!("{ASSEMBLYAI_BASE}/transcript/{id}");
    for _ in 0..ASSEMBLYAI_MAX_POLLS {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let resp = client
            .get(&poll_url)
            .header("authorization", api_key)
            .send()
            .await
            .map_err(net)?;
        let status = resp.status();
        let body = resp.text().await.map_err(net)?;
        if !status.is_success() {
            return Err(AppError::Network(format!(
                "AssemblyAI polling failed ({}): {}",
                status.as_u16(),
                provider_error_message(&body)
            )));
        }
        let v: serde_json::Value = serde_json::from_str(&body)?;
        match v.get("status").and_then(|s| s.as_str()) {
            Some("completed") => return parse_assemblyai_json(&body),
            Some("error") => {
                return Err(AppError::Network(format!(
                    "AssemblyAI transcription error: {}",
                    v.get("error").and_then(|e| e.as_str()).unwrap_or("unknown")
                )))
            }
            _ => continue, // queued | processing
        }
    }
    Err(AppError::Network(
        "AssemblyAI transcription timed out after ~20 minutes".into(),
    ))
}

/// Transcribe via Deepgram: one synchronous POST of the raw audio bytes.
pub async fn transcribe_deepgram(
    audio_path: &std::path::Path,
    api_key: &str,
    language: &str,
) -> AppResult<Transcript> {
    if api_key.trim().is_empty() {
        return Err(AppError::Validation(
            "Deepgram API key is not set — add it under Settings → API keys.".into(),
        ));
    }

    let bytes = tokio::fs::read(audio_path).await?;
    let net = |e: reqwest::Error| AppError::Network(e.to_string());

    // nova-2 + punctuate/smart_format give punctuated_word + per-word confidence.
    let mut params: Vec<(&str, &str)> = vec![
        ("model", "nova-2"),
        ("punctuate", "true"),
        ("smart_format", "true"),
    ];
    if language.is_empty() || language == "auto" {
        params.push(("detect_language", "true"));
    } else {
        params.push(("language", language));
    }

    let resp = reqwest::Client::new()
        .post("https://api.deepgram.com/v1/listen")
        .query(&params)
        .header("authorization", format!("Token {api_key}"))
        .header("content-type", mime_for(audio_path))
        .body(bytes)
        .send()
        .await
        .map_err(net)?;
    let status = resp.status();
    let body = resp.text().await.map_err(net)?;
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "Deepgram transcription failed ({}): {}",
            status.as_u16(),
            provider_error_message(&body)
        )));
    }
    parse_deepgram_json(&body)
}

/// Dispatch a cloud transcription to the chosen provider.
pub async fn cloud_transcribe(
    provider: CloudProvider,
    audio_path: &std::path::Path,
    api_key: &str,
    language: &str,
) -> AppResult<Transcript> {
    match provider {
        CloudProvider::OpenaiWhisper => transcribe_openai(audio_path, api_key, language).await,
        CloudProvider::AssemblyAi => transcribe_assemblyai(audio_path, api_key, language).await,
        CloudProvider::Deepgram => transcribe_deepgram(audio_path, api_key, language).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_error_message_extracts_or_falls_back() {
        let body =
            r#"{"error":{"message":"Invalid API key provided","type":"invalid_request_error"}}"#;
        assert_eq!(openai_error_message(body), "Invalid API key provided");
        assert_eq!(openai_error_message("not json at all"), "not json at all");
    }

    #[tokio::test]
    async fn transcribe_openai_rejects_empty_key() {
        let err = transcribe_openai(std::path::Path::new("/tmp/x.wav"), "  ", "en")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[tokio::test]
    async fn wired_providers_reject_empty_key() {
        // Every provider validates the key before any file read or network
        // call, so an empty key is a fast `validation` error through dispatch.
        for p in [
            CloudProvider::OpenaiWhisper,
            CloudProvider::AssemblyAi,
            CloudProvider::Deepgram,
        ] {
            let err = cloud_transcribe(p, std::path::Path::new("/tmp/x.wav"), "  ", "en")
                .await
                .unwrap_err();
            assert_eq!(err.code(), "validation", "provider {p:?}");
        }
    }

    #[test]
    fn mime_for_maps_common_extensions() {
        assert_eq!(mime_for(std::path::Path::new("a.wav")), "audio/wav");
        assert_eq!(mime_for(std::path::Path::new("clip.MP4")), "video/mp4");
        assert_eq!(mime_for(std::path::Path::new("x.flac")), "audio/flac");
        assert_eq!(
            mime_for(std::path::Path::new("weird.xyz")),
            "application/octet-stream"
        );
    }

    #[test]
    fn provider_error_message_handles_common_shapes() {
        assert_eq!(provider_error_message(r#"{"error":"bad key"}"#), "bad key");
        assert_eq!(
            provider_error_message(r#"{"error":{"message":"nested"}}"#),
            "nested"
        );
        assert_eq!(
            provider_error_message(r#"{"err_msg":"deepgram style"}"#),
            "deepgram style"
        );
        assert_eq!(provider_error_message("plain text"), "plain text");
    }

    #[test]
    fn consent_text_names_provider() {
        assert!(CloudProvider::AssemblyAi
            .consent_text()
            .contains("AssemblyAI"));
        assert!(CloudProvider::Deepgram.consent_text().contains("Deepgram"));
    }

    #[test]
    fn catalog_covers_every_provider_with_sane_fields() {
        let c = catalog();
        assert_eq!(c.len(), 3);
        for info in &c {
            assert!(info.price_per_min_usd > 0.0);
            assert!(info.privacy_url.starts_with("https://"));
            assert!(!info.consent_text.is_empty());
        }
    }

    #[test]
    fn only_word_level_providers_advertise_word_confidence() {
        assert!(!CloudProvider::OpenaiWhisper.has_word_confidence());
        assert!(CloudProvider::AssemblyAi.has_word_confidence());
        assert!(CloudProvider::Deepgram.has_word_confidence());
    }

    #[test]
    fn cost_scales_with_duration() {
        // 10 minutes of audio.
        let est = estimate_cost(CloudProvider::OpenaiWhisper, 600_000);
        assert!((est.minutes - 10.0).abs() < 1e-9);
        assert!((est.estimated_usd - 10.0 * 0.006).abs() < 1e-9);
    }

    #[test]
    fn cost_of_zero_or_negative_duration_is_zero() {
        assert_eq!(estimate_cost(CloudProvider::Deepgram, 0).estimated_usd, 0.0);
        assert_eq!(estimate_cost(CloudProvider::Deepgram, -5).minutes, 0.0);
    }

    // ── OpenAI ─────────────────────────────────────────────────────────────
    #[test]
    fn parses_openai_verbose_with_words() {
        let json = r#"{
          "language": "english",
          "segments": [
            { "start": 0.0, "end": 2.0, "avg_logprob": -0.02,
              "words": [
                { "word": "Hello", "start": 0.0, "end": 0.5 },
                { "word": "world", "start": 0.5, "end": 1.0 }
              ]
            }
          ]
        }"#;
        let t = parse_openai_verbose_json(json).unwrap();
        assert_eq!(t.backend, "OpenAI Whisper");
        assert_eq!(t.segments.len(), 1);
        assert_eq!(t.segments[0].words.len(), 2);
        assert_eq!(t.segments[0].words[0].text, "Hello");
        assert_eq!(t.segments[0].words[0].start_ms, 0);
        assert_eq!(t.segments[0].words[1].end_ms, 1000);
        // avg_logprob -0.02 → exp ≈ 0.98 → tier 1 (high)
        assert!(
            t.segments[0].words[0].confidence > 85.0,
            "got {}",
            t.segments[0].words[0].confidence
        );
    }

    #[test]
    fn openai_falls_back_to_segment_text_without_words() {
        let json = r#"{
          "language": "english",
          "segments": [
            { "start": 0.0, "end": 3.0, "avg_logprob": -0.5, "text": "no word timings here" }
          ]
        }"#;
        let t = parse_openai_verbose_json(json).unwrap();
        assert_eq!(t.segments[0].words.len(), 1);
        assert_eq!(t.segments[0].words[0].text, "no word timings here");
    }

    // ── AssemblyAI ─────────────────────────────────────────────────────────
    #[test]
    fn parses_assemblyai_word_confidence() {
        let json = r#"{
          "language_code": "en",
          "words": [
            { "text": "Hello", "start": 0,   "end": 500,  "confidence": 0.99 },
            { "text": "world", "start": 500, "end": 1000, "confidence": 0.55 }
          ]
        }"#;
        let t = parse_assemblyai_json(json).unwrap();
        assert_eq!(t.backend, "AssemblyAI");
        assert_eq!(t.segments[0].words.len(), 2);
        // 0.99 → tier 1, 0.55 → low tier
        assert!(t.segments[0].words[0].confidence > 85.0);
        assert!(t.segments[0].words[1].confidence < 70.0);
    }

    // ── Deepgram ───────────────────────────────────────────────────────────
    #[test]
    fn parses_deepgram_with_punctuated_words_and_seconds() {
        let json = r#"{
          "results": { "channels": [ { "detected_language": "en", "alternatives": [ {
            "words": [
              { "word": "hello", "punctuated_word": "Hello,", "start": 0.0, "end": 0.5, "confidence": 0.97 },
              { "word": "world", "punctuated_word": "world.", "start": 0.5, "end": 1.2, "confidence": 0.62 }
            ]
          } ] } ] }
        }"#;
        let t = parse_deepgram_json(json).unwrap();
        assert_eq!(t.backend, "Deepgram");
        assert_eq!(t.language, "en");
        assert_eq!(t.segments[0].words[0].text, "Hello,"); // punctuated preferred
        assert_eq!(t.segments[0].words[0].start_ms, 0);
        assert_eq!(t.segments[0].words[1].end_ms, 1200); // 1.2s → ms
        assert!(t.segments[0].words[0].confidence > 85.0);
    }

    #[test]
    fn cross_provider_confidence_parity() {
        // Same underlying probability (0.62) from AssemblyAI and Deepgram
        // must produce the same tier.
        let aai = r#"{ "words": [ { "text": "x", "start": 0, "end": 100, "confidence": 0.62 } ] }"#;
        let dg = r#"{ "results": { "channels": [ { "alternatives": [ {
          "words": [ { "word": "x", "start": 0.0, "end": 0.1, "confidence": 0.62 } ] } ] } ] } }"#;
        let a = parse_assemblyai_json(aai).unwrap().segments[0].words[0].confidence;
        let d = parse_deepgram_json(dg).unwrap().segments[0].words[0].confidence;
        assert!(
            (a - d).abs() < 0.5,
            "AssemblyAI {a} vs Deepgram {d} for same prob"
        );
    }

    #[test]
    fn rejects_malformed_responses() {
        assert!(parse_openai_verbose_json("{}").is_err());
        assert!(parse_assemblyai_json("{}").is_err());
        assert!(parse_deepgram_json("{}").is_err());
    }
}
