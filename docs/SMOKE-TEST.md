# SMOKE-TEST.md — SundayEdit

Manual end-to-end runs that the automated suite cannot cover, because they need
real hardware, a real model, real API keys, or the GUI. Each row is a single
real-world run a human performs once per release (or when the relevant code
changes). The pure logic each one exercises is already unit-tested; these rows
verify the _wiring_ against the real world.

Status legend: ☐ not yet run · ✅ verified · ⚠️ ran with issues (note them)

## ASR — Phase 2 (transcription seams)

| #   | Area                       | What to do                                                                                                                        | Expected                                                                                                                                                 | Status                                                                      |
| --- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| A1  | Local Whisper (real model) | Build `cargo build --features whisper`, download a model in-app (e.g. `large-v3-turbo`), run transcription on a short real video. | Captions appear with per-word confidence; amber/red highlights land on genuinely-uncertain words. No crash; GPU (Metal/CUDA) used when available.        | ☐ HARDWARE-UNVERIFIED                                                       |
| A2  | Local Whisper (no feature) | Default build (no `--features whisper`); attempt local transcription.                                                             | Clear, actionable error: "this build does not include local transcription — rebuild with `--features whisper`, or configure a cloud provider." No panic. | ✅ covered by unit test `local::stub_returns_actionable_error`              |
| A3  | OpenAI Whisper API         | Add a real OpenAI key in Settings → API keys; consent to upload; transcribe a clip < 25 MB.                                       | Verbose-JSON parsed into captions; backend shows "OpenAI Whisper"; word timings present; segment-level confidence applied.                               | ☐ NETWORK-UNVERIFIED                                                        |
| A4  | OpenAI oversized upload    | With OpenAI selected, point at audio > 25 MB.                                                                                     | Pre-upload `validation` error naming the 25 MB cap and suggesting local Whisper — _before_ any network call.                                             | ✅ covered by unit test `check_upload_size_message_is_clear_and_actionable` |
| A5  | AssemblyAI API             | Add a real AssemblyAI key; transcribe a clip.                                                                                     | Upload→request→poll loop completes; backend "AssemblyAI"; real per-word confidence drives the highlight tiers.                                           | ☐ NETWORK-UNVERIFIED                                                        |
| A6  | Deepgram API               | Add a real Deepgram key; transcribe a clip.                                                                                       | Single POST returns; backend "Deepgram"; punctuated words used; per-word confidence drives tiers.                                                        | ☐ NETWORK-UNVERIFIED                                                        |
| A7  | Empty/missing key          | Select any cloud provider with no key configured.                                                                                 | Fast `validation` error pointing at Settings → API keys; no file read, no network call.                                                                  | ✅ covered by unit test `wired_providers_reject_empty_key`                  |
| A8  | Cross-backend tier parity  | Transcribe the same clip locally and via a cloud provider.                                                                        | A word the model is equally (un)sure about lands in the same highlight tier across backends (confidence curve is shared).                                | ☐ NETWORK/HARDWARE-UNVERIFIED                                               |

Rows marked ☐ are P2c — see `docs/NEEDS-RICHARD.md` for what Richard needs to
supply (model download, API keys) and the exact commands.
