//! Calibration dataset generator — Phase 2.3 (deepening pass).
//!
//! Empirical calibration needs labelled words. The honest gold standard is
//! transcribing ten real videos and hand-labelling every word; that requires a
//! GPU/CPU Whisper run and human review and cannot happen in CI or headless.
//! So this generator produces a *modelled* stand-in that is:
//!
//!   - **reproducible** — fully deterministic (a fixed-seed LCG, no `rand`
//!     dependency), so anyone can re-run and get the same table;
//!   - **principled** — each of ten "videos" draws word probabilities from a
//!     two-component mixture (correct words vs. errors) whose parameters match
//!     the Whisper behaviour documented in `docs/CALIBRATION.md`: correct words
//!     cluster at p≈0.85–0.99, errors overlap downward into p≈0.40–0.85, and
//!     the error rate rises with difficulty (clean English ≈3 %, accented and
//!     Norwegian ≈9–13 %, noisy ≈16 %);
//!   - **honest** — it is NOT real measurement. The output JSON is committed as
//!     `docs/calibration-dataset.json` and clearly marked as modelled. When a
//!     real labelled set exists it replaces this file and the curve is refit
//!     against it; the harness and the refit procedure are identical either way.
//!
//! Each word carries its raw probability `p` AND the confidence the *current*
//! `confidence.rs` curve assigns it, so `cargo run --example calibrate` can
//! sweep the live curve. Refitting the curve and regenerating shows the new
//! probability→tier mapping.
//!
//!   cargo run --example calibration_dataset > ../docs/calibration-dataset.json

use serde::Serialize;
use sundayedit_lib::services::asr::confidence::provider_confidence_to_scale;

/// One modelled video: a label, a word count, an error rate, and the mixture
/// parameters for correct vs. incorrect word probabilities.
struct VideoModel {
    label: &'static str,
    words: usize,
    error_rate: f64,
    /// Correct-word probability band (lo, hi) — clusters high.
    correct_band: (f64, f64),
    /// Error-word probability band (lo, hi) — overlaps downward.
    error_band: (f64, f64),
}

/// The ten representative videos from `docs/CALIBRATION.md`, modelled.
const VIDEOS: &[VideoModel] = &[
    VideoModel {
        label: "clean English — studio podcast",
        words: 180,
        error_rate: 0.03,
        correct_band: (0.90, 0.995),
        error_band: (0.55, 0.88),
    },
    VideoModel {
        label: "clean English — solo narration",
        words: 160,
        error_rate: 0.04,
        correct_band: (0.88, 0.99),
        error_band: (0.52, 0.86),
    },
    VideoModel {
        label: "accented English — interview",
        words: 150,
        error_rate: 0.11,
        correct_band: (0.84, 0.985),
        error_band: (0.45, 0.84),
    },
    VideoModel {
        label: "accented English — conference talk",
        words: 170,
        error_rate: 0.10,
        correct_band: (0.85, 0.985),
        error_band: (0.48, 0.85),
    },
    VideoModel {
        label: "Norwegian — sermon (named terms)",
        words: 160,
        error_rate: 0.13,
        correct_band: (0.82, 0.98),
        error_band: (0.42, 0.83),
    },
    VideoModel {
        label: "Norwegian — bygdedialekt vlog",
        words: 140,
        error_rate: 0.12,
        correct_band: (0.83, 0.98),
        error_band: (0.44, 0.84),
    },
    VideoModel {
        label: "noisy — handheld outdoor",
        words: 120,
        error_rate: 0.17,
        correct_band: (0.80, 0.97),
        error_band: (0.40, 0.82),
    },
    VideoModel {
        label: "noisy — overlapping speakers",
        words: 130,
        error_rate: 0.16,
        correct_band: (0.81, 0.97),
        error_band: (0.41, 0.83),
    },
    VideoModel {
        label: "mixed — webinar with jargon",
        words: 150,
        error_rate: 0.08,
        correct_band: (0.86, 0.99),
        error_band: (0.50, 0.86),
    },
    VideoModel {
        label: "mixed — code-switching EN/NO",
        words: 140,
        error_rate: 0.10,
        correct_band: (0.84, 0.985),
        error_band: (0.46, 0.85),
    },
];

#[derive(Serialize)]
struct OutWord {
    /// Raw model probability p (= exp(avg_logprob)). Recorded so a refit can be
    /// reasoned about in probability space.
    prob: f64,
    /// Confidence the *current* `confidence.rs` curve assigns this probability.
    confidence: f32,
    correct: bool,
    video: &'static str,
}

#[derive(Serialize)]
struct Out {
    #[serde(rename = "_comment")]
    comment: &'static str,
    words: Vec<OutWord>,
}

/// Tiny deterministic LCG (Numerical Recipes constants) → no `rand` dep, fully
/// reproducible across machines.
struct Lcg(u64);
impl Lcg {
    fn next_unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // top 53 bits → [0,1)
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    /// Uniform in [lo, hi). A uniform band is the conservative choice: it makes
    /// correct and error distributions *overlap* in 0.55–0.85 (the hard region),
    /// which is exactly what makes calibration non-trivial. A peaked Beta would
    /// flatter the curve unrealistically.
    fn in_band(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_unit()
    }
}

fn main() {
    let mut lcg = Lcg(0x5EED_C0FF_EE12_3456); // fixed seed → reproducible
    let mut words = Vec::new();

    for v in VIDEOS {
        for _ in 0..v.words {
            let is_error = lcg.next_unit() < v.error_rate;
            let p = if is_error {
                lcg.in_band(v.error_band.0, v.error_band.1)
            } else {
                lcg.in_band(v.correct_band.0, v.correct_band.1)
            };
            words.push(OutWord {
                prob: p,
                confidence: provider_confidence_to_scale(p as f32),
                correct: !is_error,
                video: v.label,
            });
        }
    }

    let out = Out {
        comment: "MODELLED calibration set (not real recordings) — ten representative \
                  videos per docs/CALIBRATION.md, generated by `cargo run --example \
                  calibration_dataset`. Deterministic. Replace with real hand-labelled \
                  words when available; the refit procedure is identical.",
        words,
    };
    println!("{}", serde_json::to_string_pretty(&out).expect("serialize"));
}
