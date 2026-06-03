//! Confidence calibration harness — Phase 2.3.
//!
//! Killer feature #1 (confidence highlighting) only earns trust if low
//! confidence actually predicts errors. This module turns a set of labelled
//! words (the confidence we assigned + whether the word was actually correct)
//! into precision/recall at each candidate threshold, so the `confidence.rs`
//! curve can be fitted to the tier boundaries from data instead of guessed.
//! The shipped curve was calibrated this way (see `docs/CALIBRATION.md`);
//! `shipped_dataset_meets_calibration_target` locks the result.
//!
//! "Flagged" means `confidence < threshold` — i.e. the word the editor would
//! highlight as uncertain. Then:
//!   - precision = of flagged words, the fraction actually wrong (low = we
//!     nag the user about words that were fine).
//!   - recall = of all wrong words, the fraction we flagged (low = errors
//!     slip through unhighlighted).
//!
//! Run it on labelled data via `cargo run --example calibrate -- file.json`
//! (see `examples/calibrate.rs` and `docs/CALIBRATION.md`).

use serde::{Deserialize, Serialize};

/// One hand-labelled word: the confidence we assigned (0..100) and whether it
/// was actually correct against ground truth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LabeledWord {
    pub confidence: f32,
    pub correct: bool,
}

/// Precision/recall for treating `confidence < threshold` as the uncertainty
/// flag.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ThresholdMetrics {
    pub threshold: f32,
    pub precision: f32,
    pub recall: f32,
    pub f1: f32,
    /// How many words this threshold flags as uncertain.
    pub flagged: usize,
}

/// Compute precision/recall at one threshold.
pub fn metrics_at(words: &[LabeledWord], threshold: f32) -> ThresholdMetrics {
    let total_wrong = words.iter().filter(|w| !w.correct).count();
    let flagged = words.iter().filter(|w| w.confidence < threshold).count();
    let flagged_wrong = words
        .iter()
        .filter(|w| w.confidence < threshold && !w.correct)
        .count();

    // No flagged words → no false positives (precision is vacuously perfect).
    let precision = if flagged == 0 {
        1.0
    } else {
        flagged_wrong as f32 / flagged as f32
    };
    // No wrong words → every (zero) error was caught.
    let recall = if total_wrong == 0 {
        1.0
    } else {
        flagged_wrong as f32 / total_wrong as f32
    };
    let f1 = if precision + recall == 0.0 {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };

    ThresholdMetrics {
        threshold,
        precision,
        recall,
        f1,
        flagged,
    }
}

/// Compute metrics across a set of candidate thresholds.
pub fn sweep(words: &[LabeledWord], thresholds: &[f32]) -> Vec<ThresholdMetrics> {
    thresholds.iter().map(|&t| metrics_at(words, t)).collect()
}

/// The threshold maximizing F1 — a reasonable single "uncertain" cutoff.
/// `None` for an empty sweep.
pub fn best_f1(sweep: &[ThresholdMetrics]) -> Option<&ThresholdMetrics> {
    sweep
        .iter()
        .max_by(|a, b| a.f1.partial_cmp(&b.f1).unwrap_or(std::cmp::Ordering::Equal))
}

/// A default sweep: 50, 55, …, 95 — the band where Whisper's correct/incorrect
/// words actually separate (see `docs/CALIBRATION.md`).
pub fn default_thresholds() -> Vec<f32> {
    (10..=19).map(|i| i as f32 * 5.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(confidence: f32, correct: bool) -> LabeledWord {
        LabeledWord {
            confidence,
            correct,
        }
    }

    #[test]
    fn perfect_separation_at_threshold() {
        // All wrong words sit below 70, all correct words above.
        let words = [w(90.0, true), w(85.0, true), w(40.0, false), w(60.0, false)];
        let m = metrics_at(&words, 70.0);
        assert_eq!(m.flagged, 2);
        assert_eq!(m.precision, 1.0); // both flagged were wrong
        assert_eq!(m.recall, 1.0); // both wrong were flagged
        assert_eq!(m.f1, 1.0);
    }

    #[test]
    fn recall_is_monotonic_in_threshold() {
        let words = [
            w(95.0, true),
            w(72.0, false),
            w(55.0, false),
            w(30.0, false),
        ];
        let lo = metrics_at(&words, 50.0).recall;
        let hi = metrics_at(&words, 90.0).recall;
        assert!(
            hi >= lo,
            "raising the threshold flags more, so recall rises"
        );
        assert_eq!(metrics_at(&words, 90.0).recall, 1.0);
    }

    #[test]
    fn precision_drops_when_flagging_correct_words() {
        // At threshold 100 everything is flagged, including 2 correct words.
        let words = [w(99.0, true), w(80.0, true), w(40.0, false)];
        let m = metrics_at(&words, 100.0);
        assert_eq!(m.flagged, 3);
        assert!((m.precision - (1.0 / 3.0)).abs() < 1e-6);
        assert_eq!(m.recall, 1.0);
    }

    #[test]
    fn no_errors_gives_full_recall() {
        let words = [w(90.0, true), w(40.0, true)];
        assert_eq!(metrics_at(&words, 70.0).recall, 1.0);
    }

    #[test]
    fn best_f1_picks_the_separating_threshold() {
        let words = [w(90.0, true), w(85.0, true), w(40.0, false), w(60.0, false)];
        let table = sweep(&words, &default_thresholds());
        let best = best_f1(&table).unwrap();
        assert_eq!(best.f1, 1.0);
        // The separating cutoff is somewhere in (60, 85]; 65..=85 all score 1.
        assert!(best.threshold >= 65.0 && best.threshold <= 85.0);
    }

    #[test]
    fn default_thresholds_span_50_to_95() {
        let t = default_thresholds();
        assert_eq!(t.first(), Some(&50.0));
        assert_eq!(t.last(), Some(&95.0));
        assert_eq!(t.len(), 10);
    }

    #[test]
    fn best_f1_none_for_empty_sweep() {
        assert!(best_f1(&[]).is_none());
    }

    /// Regression lock for the shipped calibration (see `docs/CALIBRATION.md`).
    /// Loads the committed labelled set and asserts the headline claim that the
    /// app surfaces to users: flagging everything below the tier-2 floor (conf
    /// 70) catches the large majority of errors with NO false positives. If a
    /// future refit of the `confidence.rs` curve regenerates this dataset and
    /// breaks the property, this test fails loudly.
    #[test]
    fn shipped_dataset_meets_calibration_target() {
        #[derive(serde::Deserialize)]
        struct Set {
            words: Vec<LabeledWord>,
        }
        // The dataset carries extra fields (`prob`, `video`); the default serde
        // behaviour ignores unknown keys, so `LabeledWord` parses cleanly.
        let raw = include_str!("../../../../docs/calibration-dataset.json");
        let set: Set = serde_json::from_str(raw).expect("dataset parses");
        assert!(
            set.words.len() >= 1000,
            "calibration set must be substantial"
        );

        let m70 = metrics_at(&set.words, 70.0);
        assert_eq!(
            m70.precision, 1.0,
            "tier-2 flag must have no false positives, got {m70:?}"
        );
        assert!(
            m70.recall >= 0.85,
            "tier-2 flag must catch ≥85% of errors, got {}",
            m70.recall
        );

        // Best F1 over the standard sweep should land at the tier-2 floor (70),
        // i.e. the curve is fitted to the boundary we document.
        let table = sweep(&set.words, &default_thresholds());
        let best = best_f1(&table).unwrap();
        assert_eq!(
            best.threshold, 70.0,
            "best-F1 cutoff should be the tier-2 floor"
        );
    }
}
