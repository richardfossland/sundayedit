# Architecture Decision Records — Verbatim

## ADR-001 — Tauri 2, matching SundayStage

**Status:** Accepted (2026-05-28)

Same stack as SundayStage (Tauri 2 + Rust + React + TS). One toolchain serves multiple products. Tauri's small footprint matters for a tool that also runs ffmpeg + Whisper locally — we want the app shell to be lean so the heavy lifting has headroom.

## ADR-002 — Pure-function caption operations

**Status:** Accepted (2026-05-28)

Every caption edit (split, merge, edit-word, retime, shift) is a pure function `(&Project, params) -> Result<Project>`. Never mutates in place.

Consequences:
- Undo is trivial: keep the previous `Project`.
- Operations are exhaustively unit-testable without a database or UI.
- Invariants (`Project::validate`) run after every operation, so corrupt state can never be persisted.

The renderer holds project state (TanStack Query); the Rust layer is stateless for operations. When SQLite persistence lands, the same operations feed the writer.

## ADR-003 — Confidence as first-class, calibrated empirically

**Status:** Accepted (2026-05-28)

Per-word confidence (0–100) is stored on every `Word` and drives the 4-tier highlight system. This is the product's killer feature.

Tier boundaries (85 / 70 / 50) are placeholders until calibrated against real labelled transcripts (`docs/CALIBRATION.md`). The boundary logic lives in ONE place — `Word::confidence_tier()` in Rust, mirrored by `confidenceTier()` in TS — so calibration changes one constant set.

Locked or edited words are always tier 1 (no highlight) regardless of score — once a human has touched a word, we trust them.

## ADR-004 — i64 timestamps emit `number` in TS bindings

**Status:** Accepted (2026-05-28)

Rust uses `i64` milliseconds. ts-rs defaults to emitting `bigint` for i64, but Tauri's serde_json serializes i64 as a JSON number, and JS receives a `number` at runtime. The wire format is `number`, so the binding must say `number`.

We override per-field with `#[ts(type = "number")]`. JS `number` safely represents any video duration in ms (2^53 ms ≈ 285,000 years).

## ADR-005 — Local Whisper default, cloud opt-in

**Status:** Accepted (2026-05-28) — implementation pending Phase 2

The privacy + cost story. Video never leaves the machine unless the user explicitly enables a cloud provider (with a consent dialog). API keys go in the OS keychain via the `keyring` crate, never plaintext.

## ADR-006 — Stay a captioning tool; refuse scope creep

**Status:** Accepted (2026-05-28)

Verbatim is the world's best captioning tool, not the world's second-best video editor. No cuts/transitions/color-grading beyond what captions strictly need (the filler/silence ripple-edit in Phase 7.2 is the one deliberate exception, because it directly serves caption timing). Say no to 80% of feature requests in the first 12 months.
