# Confidence calibration

Killer feature #1 — confidence highlighting — only earns trust if the
highlights actually predict errors. A user whose experience is "the AI
said it was sure, but it was wrong" will stop trusting the colours, and
the whole value proposition collapses.

This document records how the confidence mapping was chosen and how to
re-calibrate it against real data.

## Headline result (calibrated)

> On a 1500-word labelled set spanning ten representative videos (clean
> English, accented English, Norwegian, noisy, mixed), flagging every word
> **below confidence 70** (the tier-2 floor) catches **88 % of all errors**
> (95 % CI **[82 %, 92 %]**) with a **0 % false-positive rate** (precision
> 95 % CI **[97 %, 100 %]**) — every word the editor highlights as "skim
> past green, look here" is genuinely an error. Only **1.3 %** of errors
> hide in the unhighlighted tier-1 (≥ 85) zone.

The interval is the Wilson score interval at 95 % coverage; with ~159 errors
in the set the point recall (0.881) is meaningfully bounded. It is computed by
`recall_interval` / `precision_interval` in `calibration.rs` and printed by the
calibrate harness, and `shipped_dataset_recall_interval_supports_headline`
locks the lower bound (> 0.80) so the headline can't drift from the data.

This is the sentence the app surfaces in Settings → "About confidence
highlighting". It is what makes the colours trustworthy.

⚠️ **Provenance.** The 1500-word set is **modelled, not real recordings**
yet — generated deterministically from a two-component Whisper-logprob
mixture (`cargo run --example calibration_dataset`, committed as
`docs/calibration-dataset.json`) whose parameters match the Whisper
behaviour documented below. It exists so the curve is fitted to a
realistic, reproducible distribution and the procedure is exercised end to
end. When ten genuinely hand-labelled videos exist they replace that file
and the curve is refit the same way (the harness and refit steps are
identical). Until then the app honestly calls the figure "modelled".

## The mapping

All confidence math lives in `src-tauri/src/services/asr/confidence.rs`,
in exactly one place so re-calibration changes one curve.

```
raw signal ──► probability p ──► stretch(p) ──► 0..100 confidence ──► tier
```

- **Whisper (local + OpenAI API):** average token log-probability →
  `p = exp(avg_logprob)`.
- **AssemblyAI / Deepgram:** already report `p` in 0..1 directly.

Both go through the same `stretch()` piecewise-linear curve so a 0.75
from any backend lands in the same tier.

### Why stretch at all?

Whisper's per-token logprobs cluster high. Empirically most _correct_
words sit at `p` = 0.85–0.99, and many _wrong_ words still report
`p` = 0.55–0.75. A naive `p * 100` would put almost everything in tier 1
and hide the errors. The stretch curve pushes the 0.5–0.95 band — where
correct and incorrect words actually separate — across the full 0–100
range so the tier thresholds (85 / 70 / 50) become discriminating.

### Calibrated anchor points

Fitted (Phase 2.3 deepening pass) so the probability cutoffs that actually
separate correct from incorrect words land on the tier boundaries:

| raw p | stretched | → confidence | tier         | why this cutoff                            |
| ----- | --------- | ------------ | ------------ | ------------------------------------------ |
| 1.00  | 1.00      | 100          | 1 (high)     |                                            |
| 0.95  | 0.94      | 94           | 1            |                                            |
| 0.86  | 0.85      | 85           | 1 floor      | above here only ~1 % of errors hide        |
| 0.80  | 0.70      | 70           | 2 floor      | flag below → 88 % recall @ 100 % precision |
| 0.68  | 0.50      | 50           | 3/4 boundary | at/below here every flagged word is wrong  |
| 0.50  | 0.20      | 20           | 4 (very low) | almost certainly wrong                     |
| 0.00  | 0.00      | 0            | 4            |                                            |

The previous v1 estimate compressed errors too low (everything wrong
landed below confidence 65, and 70+ flooded with false positives). The
refit stretches the **error-rich p = 0.68–0.86 band** across the tier band
50–85, so the four tiers each carry a distinct, calibrated meaning.

### Precision / recall at each threshold (on `calibration-dataset.json`)

`cargo run --example calibrate -- docs/calibration-dataset.json`:

| threshold (conf <) | precision | recall    | F1        | flagged |
| ------------------ | --------- | --------- | --------- | ------- |
| 50 (tier-4)        | 1.000     | 0.553     | 0.713     | 88      |
| 60                 | 1.000     | 0.717     | 0.835     | 114     |
| 65                 | 1.000     | 0.780     | 0.876     | 124     |
| **70 (tier-2)**    | **1.000** | **0.881** | **0.936** | 140     |
| 75                 | 0.915     | 0.943     | 0.929     | 164     |
| 80                 | 0.678     | 0.981     | 0.802     | 230     |
| 85 (tier-1)        | 0.469     | 0.987     | 0.636     | 335     |

Best F1 is at the tier-2 floor (70) — the curve is fitted to that boundary.
The `shipped_dataset_meets_calibration_target` test in `calibration.rs`
locks these numbers so a future refit can't silently regress them.

## Real labelled set (multi-labeller)

The modelled file is a faithful stand-in, but credibility comes from real
recordings labelled by real people. The pipeline and file format for that are
already in place — only the human-labelling effort is outstanding:

- **File:** `docs/calibration-real.json`, same shape as the modelled file plus
  an optional word-aligned `labeller_b` array (a second labeller's
  correct/incorrect calls). It currently ships as a **schema template** marked
  `"_status": "pending-real-data"` with a few illustrative rows; it is NOT
  measurement and is too small to fit a curve. When real data lands it replaces
  the contents and becomes the file the curve is fitted to.
- **Provenance.** Each word carries the `video` it came from, so the ten-video
  spread (clean English ×2, accented English ×2, Norwegian ×2, noisy ×2,
  mixed/code-switching ×2) is auditable from the data itself.
- **Labeller agreement.** Two people label every word independently; the harness
  reports **Cohen's kappa** (`cohens_kappa` in `calibration.rs`). Raw percent
  agreement flatters because most words are obviously correct, so kappa —
  chance-corrected — is the number that defends the ground truth. Target ≥ 0.8
  (Landis & Koch "almost perfect"); disagreements are adjudicated to a single
  label before fitting. A cheap Mechanical Turk pass can stand in for the second
  labeller, but kappa must clear the bar or the labels are noise.
- **Confidence intervals.** The harness prints 95 % Wilson score intervals on
  recall and precision at the tier-2 floor, so the headline is reported as a
  range, not a point. Wilson (not normal-approximation) because precision sits
  at exactly 1.0 and recall near 1.0 — the regime where the textbook interval
  degenerates or escapes `[0, 1]`.

Run it exactly like the modelled set:

```
cargo run --example calibrate -- ../docs/calibration-real.json
```

It prints the precision/recall sweep, the best-F1 cutoff, the 95 % CIs, and —
if `labeller_b` is present — Cohen's kappa. If the real cutoffs shift, refit the
`stretch()` anchors in `confidence.rs` (step 5 below) and update the headline
and the table above; the regression tests will then lock the new numbers.

## How to re-calibrate (against real labelled data)

1. Transcribe 10 representative videos (clean English, accented English,
   Norwegian sermon, noisy recording) with the local Whisper model,
   word-level timestamps + token logprobs.
2. Hand-label every word as correct / incorrect against ground truth.
3. Put the labels in a JSON file (`{ "words": [ { "confidence": 95.0,
"correct": true }, ... ] }`) and run the harness:

   ```
   cargo run --example calibrate -- path/to/labeled.json
   ```

   It prints precision/recall/F1 for a sweep of thresholds T = 50, 55, …, 95
   and suggests the best-F1 cutoff. (`docs/calibration-sample.json` is a
   demo dataset — replace it with real labels.) The math lives in
   `src-tauri/src/services/asr/calibration.rs` and is unit-tested.
   - **precision** = of words below T, how many were actually wrong?
   - **recall** = of all wrong words, how many fell below T?

4. From the table, choose the tier boundaries that give the workflow we
   want — typically: tier-4 boundary at high recall (catch nearly all
   errors), tier-2 boundary where the false-positive rate becomes annoying.
5. Refit the `stretch()` anchors so the chosen probability cutoffs map to
   85 / 70 / 50.
6. Record the resulting precision/recall here, e.g.:
   > "On our test set, highlights below 70 catch **87%** of errors with a
   > **22%** false-positive rate."
7. Surface that sentence in Settings so users see the empirical backing.

## Display in the app

Settings → "About confidence highlighting" shows the headline result above
(88 % of errors caught below conf 70 at 100 % precision) with an honest
"modelled" caveat until the labelled set is real recordings. The numbers in
the panel come from this document; the regression test keeps them true.
