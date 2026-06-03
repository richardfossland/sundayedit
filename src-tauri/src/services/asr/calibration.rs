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

// ── Statistical rigour for a *finite* labelled sample ─────────────────────────
//
// Precision and recall computed on 1500 hand-labelled words are point estimates
// of an unknown true rate; with only ~140 errors a single number oversells the
// certainty. A headline like "catches 88 % of errors" needs an interval. We use
// the Wilson score interval (better than normal-approximation for proportions
// near 0/1 and small denominators, which is exactly the recall-at-100%-precision
// regime). And when two labellers independently mark the same words, raw percent
// agreement flatters because most words are obviously correct — Cohen's kappa
// corrects for chance agreement. Both are documented in `docs/CALIBRATION.md`.

/// A two-sided confidence interval for a proportion (e.g. recall), 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ConfidenceInterval {
    pub point: f32,
    pub lower: f32,
    pub upper: f32,
}

/// Wilson score interval for `successes` out of `n` Bernoulli trials.
///
/// `z` is the standard-normal quantile for the desired coverage (1.96 ≈ 95 %).
/// Unlike the textbook `p ± z·√(p(1-p)/n)` it stays inside `[0, 1]` and does not
/// collapse to a zero-width interval when `p` hits 0 or 1 — both of which happen
/// in calibration (recall near 1, precision exactly 1). `n == 0` yields the full
/// `[0, 1]` interval (no information).
pub fn wilson_interval(successes: usize, n: usize, z: f32) -> ConfidenceInterval {
    if n == 0 {
        return ConfidenceInterval {
            point: 0.0,
            lower: 0.0,
            upper: 1.0,
        };
    }
    let n_f = n as f32;
    let p = successes as f32 / n_f;
    let z2 = z * z;
    let denom = 1.0 + z2 / n_f;
    let centre = (p + z2 / (2.0 * n_f)) / denom;
    let margin = (z / denom) * ((p * (1.0 - p) / n_f) + z2 / (4.0 * n_f * n_f)).sqrt();
    ConfidenceInterval {
        point: p,
        lower: (centre - margin).clamp(0.0, 1.0),
        upper: (centre + margin).clamp(0.0, 1.0),
    }
}

/// 95 % Wilson interval on *recall* at a threshold: of the wrong words, the
/// fraction flagged (`confidence < threshold`). The denominator is the number of
/// errors, which is why the interval matters — there are far fewer errors than
/// words, so the recall estimate is the loose one.
pub fn recall_interval(words: &[LabeledWord], threshold: f32) -> ConfidenceInterval {
    let total_wrong = words.iter().filter(|w| !w.correct).count();
    let caught = words
        .iter()
        .filter(|w| !w.correct && w.confidence < threshold)
        .count();
    wilson_interval(caught, total_wrong, 1.96)
}

/// 95 % Wilson interval on *precision* at a threshold: of the flagged words, the
/// fraction actually wrong. Denominator is the number of flagged words.
pub fn precision_interval(words: &[LabeledWord], threshold: f32) -> ConfidenceInterval {
    let flagged = words.iter().filter(|w| w.confidence < threshold).count();
    let flagged_wrong = words
        .iter()
        .filter(|w| w.confidence < threshold && !w.correct)
        .count();
    wilson_interval(flagged_wrong, flagged, 1.96)
}

/// Cohen's kappa for two labellers each making a binary correct/incorrect call
/// on the same `n` words. Returns `None` if the inputs differ in length or are
/// empty. Range: 1.0 = perfect agreement, 0.0 = chance-level, negative = worse
/// than chance. Landis & Koch read: > 0.8 almost perfect, 0.6–0.8 substantial.
///
/// This is the number that defends the labels: if two independent humans only
/// agree by chance, the "ground truth" is noise and the curve is fitted to it.
pub fn cohens_kappa(labeller_a: &[bool], labeller_b: &[bool]) -> Option<f32> {
    if labeller_a.len() != labeller_b.len() || labeller_a.is_empty() {
        return None;
    }
    let n = labeller_a.len() as f32;
    let agree = labeller_a
        .iter()
        .zip(labeller_b)
        .filter(|(a, b)| a == b)
        .count() as f32;
    let observed = agree / n;

    // Expected agreement by chance, from each labeller's marginal "true" rate.
    let a_true = labeller_a.iter().filter(|&&x| x).count() as f32 / n;
    let b_true = labeller_b.iter().filter(|&&x| x).count() as f32 / n;
    let expected = a_true * b_true + (1.0 - a_true) * (1.0 - b_true);

    if (1.0 - expected).abs() < f32::EPSILON {
        // Both labellers marked everything the same class → agreement is trivial
        // and kappa is undefined; report perfect agreement only if they match.
        return Some(if (observed - 1.0).abs() < f32::EPSILON {
            1.0
        } else {
            0.0
        });
    }
    Some((observed - expected) / (1.0 - expected))
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

    // ── Wilson interval ───────────────────────────────────────────────────────
    #[test]
    fn wilson_zero_n_is_no_information() {
        let ci = wilson_interval(0, 0, 1.96);
        assert_eq!(ci.lower, 0.0);
        assert_eq!(ci.upper, 1.0);
    }

    #[test]
    fn wilson_stays_inside_unit_interval_at_extremes() {
        // p = 1 (everything a success) must NOT give a degenerate [1, 1].
        let all = wilson_interval(40, 40, 1.96);
        assert_eq!(all.point, 1.0);
        assert!(all.lower < 1.0, "upper-bounded estimate, got {all:?}");
        assert!((all.upper - 1.0).abs() < 1e-6);
        // p = 0 likewise.
        let none = wilson_interval(0, 40, 1.96);
        assert_eq!(none.point, 0.0);
        assert!(none.lower.abs() < 1e-6);
        assert!(none.upper > 0.0, "lower-bounded estimate, got {none:?}");
    }

    #[test]
    fn wilson_brackets_the_point_estimate_and_narrows_with_n() {
        let small = wilson_interval(7, 10, 1.96);
        let large = wilson_interval(700, 1000, 1.96);
        assert!(small.lower <= small.point && small.point <= small.upper);
        assert!(large.lower <= large.point && large.point <= large.upper);
        // Same proportion (0.7) but 100× the data → a tighter interval.
        let small_w = small.upper - small.lower;
        let large_w = large.upper - large.lower;
        assert!(
            large_w < small_w,
            "more data narrows the interval: {large_w} vs {small_w}"
        );
    }

    #[test]
    fn recall_interval_brackets_point_recall() {
        // 6 wrong, 5 of them flagged below 70 → point recall 5/6.
        let words = [
            w(95.0, true),
            w(40.0, false),
            w(55.0, false),
            w(60.0, false),
            w(65.0, false),
            w(50.0, false),
            w(80.0, false), // wrong but ABOVE 70 → missed
        ];
        let pr = metrics_at(&words, 70.0).recall;
        let ci = recall_interval(&words, 70.0);
        assert!(
            (ci.point - pr).abs() < 1e-6,
            "interval point == metric recall"
        );
        assert!(ci.lower <= pr && pr <= ci.upper);
    }

    #[test]
    fn precision_interval_brackets_point_precision() {
        let words = [w(40.0, false), w(60.0, false), w(50.0, true)]; // 2/3 flagged-wrong
        let pp = metrics_at(&words, 70.0).precision;
        let ci = precision_interval(&words, 70.0);
        assert!((ci.point - pp).abs() < 1e-6);
        assert!(ci.lower <= pp && pp <= ci.upper);
    }

    // ── Cohen's kappa ─────────────────────────────────────────────────────────
    #[test]
    fn kappa_perfect_agreement_is_one() {
        let a = [true, false, true, false, true];
        let b = [true, false, true, false, true];
        assert!((cohens_kappa(&a, &b).unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn kappa_rejects_mismatched_or_empty() {
        assert!(cohens_kappa(&[true], &[true, false]).is_none());
        assert!(cohens_kappa(&[], &[]).is_none());
    }

    #[test]
    fn kappa_substantial_when_one_disagreement() {
        // 9/10 agree; disagreement is on the rare "incorrect" class.
        let a = [true, true, true, true, true, true, true, true, false, false];
        let b = [true, true, true, true, true, true, true, true, false, true];
        let k = cohens_kappa(&a, &b).unwrap();
        // High raw agreement but the rare class drags kappa below it — exactly
        // why we report kappa instead of percent agreement.
        assert!(k < 0.9, "chance-corrected below raw agreement, got {k}");
        assert!(k > 0.5, "still substantial, got {k}");
    }

    #[test]
    fn kappa_handles_single_class() {
        // Both labellers marked everything correct → undefined expected; we
        // report 1.0 when they match exactly.
        let a = [true, true, true];
        let b = [true, true, true];
        assert_eq!(cohens_kappa(&a, &b), Some(1.0));
    }

    /// The headline result the app surfaces is a single number, but the honest
    /// claim is a *range*. This locks the 95 % recall interval at the tier-2
    /// floor on the shipped set, so CALIBRATION.md's interval can't silently
    /// drift from the code.
    #[test]
    fn shipped_dataset_recall_interval_supports_headline() {
        #[derive(serde::Deserialize)]
        struct Set {
            words: Vec<LabeledWord>,
        }
        let raw = include_str!("../../../../docs/calibration-dataset.json");
        let set: Set = serde_json::from_str(raw).expect("dataset parses");
        let ci = recall_interval(&set.words, 70.0);
        // Headline is "≈88 %"; the 95 % interval must comfortably exclude a
        // pessimistic "only catches half the errors" reading.
        assert!(
            ci.lower > 0.80,
            "even the lower bound beats 80% recall, got {ci:?}"
        );
        assert!(ci.point >= 0.85);
    }
}
