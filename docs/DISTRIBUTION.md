# Distribution & auto-update (Phase 9.2)

SundayEdit ships as signed installers for macOS and Windows, with auto-update
over GitHub Releases. This doc is the checklist to make the pipeline live —
the code/config is already in place, what's left is **secrets and accounts**
only you can provide.

## How it works

1. You push a version tag (`vX.Y.Z`).
2. `.github/workflows/release.yml` builds on macOS (universal) and Windows,
   signs + notarizes (macOS), signs the updater bundle, and creates a
   **draft** GitHub Release with the installers and `latest.json`.
3. You review the draft and publish it.
4. Installed apps check `https://github.com/richardfossland/sundayedit/releases/latest/download/latest.json`
   on launch, verify the signature against the public key embedded in
   `tauri.conf.json`, and offer the update (the in-app banner).

The updater **public** key is committed in `tauri.conf.json`
(`plugins.updater.pubkey`). The matching **private** key was generated to
`~/.tauri/sundayedit_updater.key` (with an empty password) and is NOT in the
repo. Keep it safe — losing it means existing installs can no longer
auto-update (they'd need a manual reinstall with a new key).

## Required GitHub repository secrets

Settings → Secrets and variables → Actions → New repository secret.

### Updater (required — without these the release build fails)

| Secret                               | Value                                                                                                                                                              |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `TAURI_SIGNING_PRIVATE_KEY`          | Contents of `~/.tauri/sundayedit_updater.key`. Copy without echoing it: `cat ~/.tauri/sundayedit_updater.key \| pbcopy`, then paste.                               |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The password for that key. It was generated with an **empty** password, so set this secret to an empty string (or regenerate the key with a password — see below). |

### macOS code signing + notarization (you already do this for SundayRec)

| Secret                       | Value                                                                                                                        |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Base64 of your "Developer ID Application" cert exported as `.p12`: `base64 -i cert.p12 \| pbcopy`.                           |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12`.                                                                              |
| `APPLE_SIGNING_IDENTITY`     | e.g. `Developer ID Application: Richard Fossland (TEAMID)`. Find it with `security find-identity -v -p codesigning`.         |
| `APPLE_ID`                   | Your Apple Developer account email.                                                                                          |
| `APPLE_PASSWORD`             | An **app-specific password** (appleid.apple.com → Sign-In and Security → App-Specific Passwords), not your account password. |
| `APPLE_TEAM_ID`              | Your 10-character Apple Team ID.                                                                                             |

### Windows code signing (deferred — see below)

Not wired yet. The pipeline currently produces an **unsigned** Windows
installer (it still works, but SmartScreen will warn). To add it, see
"Windows signing" below and add the relevant secrets.

## Cut a release

```bash
# bump version in package.json AND src-tauri/tauri.conf.json + Cargo.toml
# (keep them in sync), then:
git tag v0.2.0
git push origin v0.2.0
```

Watch the `release` workflow, then publish the draft Release it created.

> First release: there is no `latest.json` yet, so `checkForUpdate()` simply
> returns null in older builds — expected. Auto-update kicks in from the
> release _after_ the first one.

## Regenerating the updater key with a password (optional, recommended)

```bash
npx tauri signer generate -w ~/.tauri/sundayedit_updater.key -f   # prompts for a password
```

Then update `plugins.updater.pubkey` in `tauri.conf.json` with the new
public key, and update the `TAURI_SIGNING_PRIVATE_KEY` /
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets. Note: rotating the key means
apps signed with the old key won't accept updates signed with the new one —
only do this before you have real users.

## ffmpeg sidecar (wired)

ffmpeg + ffprobe are bundled as Tauri `externalBin` sidecars, so import,
waveform, and burn-in work without a system ffmpeg:

- `bundle.externalBin` in `tauri.conf.json` lists `binaries/ffmpeg` +
  `binaries/ffprobe`.
- At runtime `services/video.rs` resolves the binary next to the app
  executable first (the bundled sidecar), then falls back to PATH, with a
  `SUNDAYEDIT_FFMPEG` / `SUNDAYEDIT_FFPROBE` env override for dev/tests.
- The binaries are **not committed** (too large). Fetch them before every
  build: `node scripts/fetch-ffmpeg.mjs` — it copies the `ffmpeg-static` +
  `@ffprobe-installer/ffprobe` binaries into `src-tauri/binaries/` with the
  Rust target-triple suffix. CI runs this automatically (see
  `release.yml`); each runner fetches its own platform's binaries.

> **Licensing:** these are GPL/LGPL ffmpeg builds. Before any _public_
> release, confirm GPL compliance (offer the corresponding source) or swap to
> an LGPL/own build. Fine for private test builds.

> macOS builds are currently **arm64-only** (the fetch script + CI build the
> runner's native arch). Intel/universal mac is a follow-up — it needs both
> `ffmpeg-x86_64-apple-darwin` and `ffmpeg-aarch64-apple-darwin` present.

## Deferred — required before a real public 1.0

These are intentionally **not** wired yet (they need binaries/infra, not just
config):

- ~~**Whisper model download.**~~ ✅ Done — the app downloads the chosen ggml
  model on first run from the Hugging Face `whisper.cpp` repo
  (`asr_download_model`: atomic `.part`→rename, progress events, cancel, size
  verify), driven by the model picker / onboarding. No bundling, no own CDN.
- **Windows code signing.** Import a code-signing cert on the runner (or use
  Azure Trusted Signing) and set `bundle.windows.certificateThumbprint` /
  the signing env. EV cert avoids SmartScreen warnings.
- **Beta channel.** A second updater endpoint + `prerelease` releases for
  opt-in testers.
- **End-to-end update test.** Build vX, install it, publish vX+1, confirm the
  in-app banner downloads + relaunches into the new version before any public
  announcement.

## Local production build (unsigned, for smoke-testing the bundle)

```bash
npm ci
node scripts/fetch-ffmpeg.mjs  # bundle ffmpeg/ffprobe (required by externalBin)
npm run tauri build            # add --features llm,whisper,diarize for full native AI
```

Artifacts land in `src-tauri/target/release/bundle/`.
