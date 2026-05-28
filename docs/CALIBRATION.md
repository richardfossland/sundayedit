# Confidence calibration

Killer feature #1 — confidence highlighting — only earns trust if the
highlights actually predict errors. A user whose experience is "the AI
said it was sure, but it was wrong" will stop trusting the colours, and
the whole value proposition collapses.

This document records how the confidence mapping was chosen and how to
re-calibrate it against real data.

## The mapping (current: v1 estimate)

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

### Current anchor points (v1, UNCALIBRATED)

| raw p | stretched | → confidence | tier         |
| ----- | --------- | ------------ | ------------ |
| 1.00  | 1.00      | 100          | 1 (high)     |
| 0.95  | 0.88      | 88           | 1            |
| 0.88  | 0.72      | 72           | 2 (medium)   |
| 0.75  | 0.55      | 55           | 3 (low)      |
| 0.50  | 0.30      | 30           | 4 (very low) |
| 0.00  | 0.00      | 0            | 4            |

These are an **educated guess**, not fitted to data. They will be wrong
in detail. The structure (single curve, single source of truth) is what
matters until we have labelled transcripts.

## How to re-calibrate (the real work, Phase 2.3)

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

Settings → "About confidence highlighting" should show the calibration
numbers from step 6 once they exist. Until then it honestly says the
feature uses an uncalibrated v1 estimate.
