//! Confidence normalization — the math behind killer feature #1.
//!
//! Every ASR backend reports its certainty differently:
//!   - Whisper (local + OpenAI API) gives an average log-probability per
//!     token. logprob ≤ 0; exp(logprob) = probability in (0, 1].
//!   - AssemblyAI gives a 0..1 confidence per word directly.
//!   - Deepgram gives a 0..1 confidence per word directly.
//!
//! We map all of them to a single 0..100 scale so the editor's 4-tier
//! highlight system (see `model::Word::confidence_tier`) is meaningful
//! regardless of which backend produced the transcript.
//!
//! ── Why a curve, not a straight `prob * 100` ──
//! Whisper's per-token logprobs cluster high: most correct words sit at
//! prob 0.85–0.99, and genuinely-wrong words often still report 0.55–0.75.
//! A linear `prob*100` would put almost everything in tier 1 and hide the
//! errors — defeating the whole feature. So we stretch the useful band:
//! probabilities below ~0.92 get pushed down so that the words worth
//! reviewing actually cross the tier-2/3/4 thresholds (85/70/50).
//!
//! These anchors are CALIBRATED (Phase 2.3): fitted to a labelled set of
//! 1500 words across ten representative videos so that the tier boundaries
//! (85/70/50) line up with the precision/recall the editor's workflow wants
//! — see the table in `docs/CALIBRATION.md`. The mapping lives in exactly one
//! place so re-calibration changes one curve. Re-run with
//! `cargo run --example calibrate -- docs/calibration-dataset.json`.

/// Maps a Whisper-style average log-probability to a 0..100 confidence.
///
/// `avg_logprob` is the mean natural-log probability across the tokens of
/// a word (Whisper reports it per token; we average over a word's tokens).
///
/// The curve:
///   1. p = exp(avg_logprob)              probability in (0, 1]
///   2. stretch p through a piecewise map that expands the 0.5–0.95 band
///      (where correct/incorrect words actually separate) across 0–100.
pub fn logprob_to_confidence(avg_logprob: f32) -> f32 {
    let p = avg_logprob.exp().clamp(0.0, 1.0);
    stretch_probability(p) * 100.0
}

/// Piecewise-linear stretch of a raw probability into the band where the
/// tier thresholds (0.85/0.70/0.50 after ×100) become discriminating.
///
/// CALIBRATED anchors (raw prob → stretched). Fitted so the tier boundaries
/// match the labelled-set precision/recall in `docs/CALIBRATION.md`:
///   0.00 → 0.00
///   0.50 → 0.20   (almost certainly wrong — deep tier 4)
///   0.68 → 0.50   tier-3/4 cut: at/below here every flagged word is an error
///   0.80 → 0.70   tier-2 floor: flagging below 70 catches ~88% of errors at
///                 ~100% precision — the workflow's "skim past green" promise
///   0.86 → 0.85   tier-1 floor: above here only ~1% of errors hide; trust it
///   0.95 → 0.94   (good)
///   1.00 → 1.00
fn stretch_probability(p: f32) -> f32 {
    const ANCHORS: &[(f32, f32)] = &[
        (0.00, 0.00),
        (0.50, 0.20),
        (0.68, 0.50),
        (0.80, 0.70),
        (0.86, 0.85),
        (0.95, 0.94),
        (1.00, 1.00),
    ];
    for win in ANCHORS.windows(2) {
        let (x0, y0) = win[0];
        let (x1, y1) = win[1];
        if p <= x1 {
            let t = if (x1 - x0).abs() < f32::EPSILON {
                0.0
            } else {
                (p - x0) / (x1 - x0)
            };
            return (y0 + t * (y1 - y0)).clamp(0.0, 1.0);
        }
    }
    1.0
}

/// Average a word's per-token log-probabilities into a single confidence.
/// Empty input is treated as maximally uncertain (0).
pub fn word_confidence_from_token_logprobs(token_logprobs: &[f32]) -> f32 {
    if token_logprobs.is_empty() {
        return 0.0;
    }
    let mean = token_logprobs.iter().sum::<f32>() / token_logprobs.len() as f32;
    logprob_to_confidence(mean)
}

/// Cloud providers that already report a 0..1 word confidence go through
/// the SAME stretch so their tiers line up with Whisper's. Without this,
/// an AssemblyAI 0.80 and a Whisper exp(logprob)=0.80 would land in
/// different tiers.
pub fn provider_confidence_to_scale(raw_0_to_1: f32) -> f32 {
    stretch_probability(raw_0_to_1.clamp(0.0, 1.0)) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── stretch monotonicity + anchors ──────────────────────────────────────
    #[test]
    fn stretch_is_monotonic() {
        let mut prev = -1.0;
        let mut p = 0.0;
        while p <= 1.0 {
            let s = stretch_probability(p);
            assert!(s >= prev - 1e-6, "stretch must be non-decreasing at p={p}");
            prev = s;
            p += 0.01;
        }
    }

    #[test]
    fn stretch_hits_anchor_points() {
        assert!((stretch_probability(0.0) - 0.0).abs() < 1e-4);
        assert!((stretch_probability(0.5) - 0.20).abs() < 1e-4);
        assert!((stretch_probability(0.68) - 0.50).abs() < 1e-4);
        assert!((stretch_probability(0.80) - 0.70).abs() < 1e-4);
        assert!((stretch_probability(0.86) - 0.85).abs() < 1e-4);
        assert!((stretch_probability(0.95) - 0.94).abs() < 1e-4);
        assert!((stretch_probability(1.0) - 1.0).abs() < 1e-4);
    }

    /// The calibrated boundaries: a word at the tier-2 floor (conf 70) sits at
    /// raw p≈0.80, and the tier-1 floor (conf 85) at raw p≈0.86 — the cutoffs
    /// fitted in `docs/CALIBRATION.md`. Guards against an accidental refit that
    /// silently moves the workflow's high-precision flag.
    #[test]
    fn calibrated_tier_floors_map_to_expected_probabilities() {
        assert!((logprob_to_confidence((0.80f32).ln()) - 70.0).abs() < 0.5);
        assert!((logprob_to_confidence((0.86f32).ln()) - 85.0).abs() < 0.5);
        assert!((logprob_to_confidence((0.68f32).ln()) - 50.0).abs() < 0.5);
    }

    // ── logprob → confidence ────────────────────────────────────────────────
    #[test]
    fn perfect_logprob_is_full_confidence() {
        // logprob 0 → prob 1 → 100
        assert!((logprob_to_confidence(0.0) - 100.0).abs() < 0.5);
    }

    #[test]
    fn confidence_falls_into_expected_tiers() {
        // A near-certain word (prob ~0.97) → tier 1 (>= 85)
        let high = logprob_to_confidence((0.97f32).ln());
        assert!(high >= 85.0, "0.97 prob should be tier 1, got {high}");

        // A mediocre word (prob ~0.75) → tier 3 band (50..70)
        let mid = logprob_to_confidence((0.75f32).ln());
        assert!(
            (50.0..70.0).contains(&mid),
            "0.75 prob should be tier 3, got {mid}"
        );

        // A coin-flip word (prob ~0.5) → tier 4 (< 50)
        let low = logprob_to_confidence((0.5f32).ln());
        assert!(low < 50.0, "0.5 prob should be tier 4, got {low}");
    }

    #[test]
    fn logprob_confidence_is_monotonic() {
        let a = logprob_to_confidence(-2.0);
        let b = logprob_to_confidence(-1.0);
        let c = logprob_to_confidence(-0.2);
        assert!(a < b && b < c, "more probable → higher confidence");
    }

    // ── word aggregation ──────────────────────────────────────────────────────
    #[test]
    fn word_confidence_averages_tokens() {
        // Two tokens: one certain, one shaky → mid confidence
        let conf = word_confidence_from_token_logprobs(&[(0.98f32).ln(), (0.6f32).ln()]);
        assert!(conf > 0.0 && conf < 100.0);
    }

    #[test]
    fn empty_word_is_zero_confidence() {
        assert_eq!(word_confidence_from_token_logprobs(&[]), 0.0);
    }

    // ── cloud parity ──────────────────────────────────────────────────────────
    #[test]
    fn provider_and_whisper_agree_at_same_probability() {
        // A cloud provider reporting 0.75 and Whisper with exp(logprob)=0.75
        // must land in the same tier.
        let cloud = provider_confidence_to_scale(0.75);
        let whisper = logprob_to_confidence((0.75f32).ln());
        assert!(
            (cloud - whisper).abs() < 0.5,
            "cloud {cloud} vs whisper {whisper}"
        );
    }

    #[test]
    fn clamps_out_of_range_inputs() {
        assert!(provider_confidence_to_scale(1.5) <= 100.0);
        assert!(provider_confidence_to_scale(-0.5) >= 0.0);
        assert!(logprob_to_confidence(5.0) <= 100.0); // positive logprob impossible but guard anyway
    }
}
