//! Confidence calibration runner — Phase 2.3.
//!
//! Turns hand-labelled words into a precision/recall table so the confidence
//! tier boundaries can be chosen from data instead of guessed.
//!
//!   cargo run --example calibrate -- path/to/labeled.json
//!
//! Input JSON:
//!   { "words": [ { "confidence": 95.0, "correct": true }, ... ] }
//!
//! `confidence` is the 0..100 score Verbatim assigned; `correct` is whether
//! the word matched ground truth on manual review. See docs/CALIBRATION.md
//! for the full procedure (and docs/calibration-sample.json for a demo).

use std::process::ExitCode;

use serde::Deserialize;
use verbatim_lib::services::asr::calibration::{best_f1, default_thresholds, sweep, LabeledWord};

#[derive(Deserialize)]
struct Input {
    words: Vec<LabeledWord>,
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: cargo run --example calibrate -- <labeled.json>");
        return ExitCode::FAILURE;
    };

    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let input: Input = match serde_json::from_str(&raw) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("could not parse {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let total = input.words.len();
    let wrong = input.words.iter().filter(|w| !w.correct).count();
    println!("{total} labelled words, {wrong} wrong ({:.1}% error rate)\n", pct(wrong, total));

    let table = sweep(&input.words, &default_thresholds());
    println!("threshold  precision   recall      f1   flagged");
    println!("--------------------------------------------------");
    for m in &table {
        println!(
            "{:>8.0}   {:>8.3}  {:>8.3}  {:>6.3}  {:>7}",
            m.threshold, m.precision, m.recall, m.f1, m.flagged,
        );
    }

    if let Some(best) = best_f1(&table) {
        println!(
            "\nBest F1 at threshold {:.0}: precision {:.3}, recall {:.3}.",
            best.threshold, best.precision, best.recall,
        );
        println!(
            "→ Use ~{:.0} as the 'uncertain' cutoff and refit the stretch() \
             anchors in confidence.rs so it maps to the tier boundary you want \
             (see docs/CALIBRATION.md).",
            best.threshold,
        );
    }

    ExitCode::SUCCESS
}

fn pct(n: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        100.0 * n as f32 / total as f32
    }
}
