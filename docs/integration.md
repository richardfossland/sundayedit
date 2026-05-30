# Sunday-link integration — Phase 8

SundayEdit is a standalone product, but it can accept a hand-off from a sister
Sunday-suite app (SundayRec today, SundayStage later) so a recording flows
straight from "stop recording" to "caption it" with the context already filled
in. This is opt-in on the caller's side and additive on ours: a user who never
uses another Sunday app sees nothing change.

## Inbound deep link

SundayEdit registers the `sundayedit://` URL scheme (see
`tauri.conf.json` → `plugins.deep-link.desktop.schemes`). A caller launches us
with:

```text
sundayedit://import
  ?path=<absolute path to the source video/audio, REQUIRED>
  &language=<ISO code, optional>           e.g. no
  &context=<free-text priming, optional>   e.g. Sermon, speaker: Ola Nordmann
  &glossary=<comma-separated terms>        e.g. Ola Nordmann,kerygma
  &returnTo=<caller scheme, optional>      e.g. sundayrec
```

All values are percent-encoded (`+` is also accepted for spaces). Unknown query
keys are ignored, so the contract can grow without breaking older builds. `path`
is mandatory; everything else is optional. `language`/`return_to` also accept
the aliases `lang`/`return_to`.

> **Naming note:** the pre-rebrand SundayRec integration plan refers to the
> scheme as `verbatim://`. After the rebrand SundayEdit owns `sundayedit://`;
> the SundayRec side should emit `sundayedit://import?…`.

## What happens on receipt

1. The native layer (`src-tauri/src/lib.rs`, deep-link plugin) emits the raw URL
   to the renderer on the `deep-link://import` event.
2. The renderer calls the `deeplink_parse_import` command, which runs the pure
   parser in `src-tauri/src/services/deeplink.rs` and returns a validated
   `ImportRequest` (or a `validation` error).
3. `App.tsx` creates a project from `path` the normal way
   (`project_create_from_video`), then `seedProjectFromImport`
   (`src/features/project/deepLinkImport.ts`) folds in:
   - `language` → transcription language
   - `context` → `context_description` (killer feature #2 priming)
   - `glossary` → new `GlossaryTerm`s (speaker names, jargon), de-duplicated
     case-insensitively against any existing terms
4. The user lands on the **Transcribe** tab, ready to run.

## Return path (not yet implemented)

`returnTo` is parsed and carried but not yet acted on. The intended contract:
once captions are exported, write a predictable sidecar next to the source video
(`<video>.srt` / `.vtt`) and/or call `<returnTo>://captions?path=…` back so the
caller can pick the result up. Tracked as Phase 8 follow-up.

## Status

- **Done & tested headlessly:** URL parser (9 Rust unit tests), `ImportRequest`
  binding, `deeplink_parse_import` command, renderer seeding (5 vitest), event
  wiring, scheme + capability config.
- **Needs native verification on Richard's machine:** the OS scheme round-trip
  (launch via `open sundayedit://import?…` on macOS / a registered handler on
  Windows) and cold-start handoff. Deep-link OS registration only takes effect
  from a bundled build, not `vite` browser dev.
