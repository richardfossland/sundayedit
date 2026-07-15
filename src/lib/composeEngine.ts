/**
 * Compose-engine helpers (Task U) — the thin, non-`ipc.ts` glue the multi-track
 * export + preview-render UI needs.
 *
 * `ipc.compose.render` / `ipc.compose.cancel` already live in `ipc.ts` (owned by
 * the Rust agent). This module only adds the pieces the frontend phase needs and
 * that `ipc.ts` doesn't own yet:
 *
 *   - `subscribeComposeProgress` — the render command streams `ComposeProgress`
 *     on the `compose-render-progress` event. Under Tauri that's the Tauri event
 *     bus; the browser/E2E mock re-emits it as a `window` CustomEvent (it can't
 *     reproduce the bus). We listen to BOTH so the progress modal works in the
 *     real app and in Playwright without reimplementing Tauri's event layer.
 *   - `renderPreviewProxy` — invokes `compose_preview_proxy` directly (the Rust
 *     command lands this phase); guarded so it degrades to a no-op off-Tauri.
 *   - `defaultComposeSettings` — project-derived H264/CPU defaults.
 */

import { invoke, isTauri } from "@tauri-apps/api/core";
import type { ComposeProgress, ComposeSettings, Project } from "./bindings";

export const COMPOSE_PROGRESS_EVENT = "compose-render-progress";

/**
 * Subscribe to compose-render progress. Returns an unsubscribe function. Listens
 * to the `window` CustomEvent (browser/E2E) and, under Tauri, the Tauri event of
 * the same name — whichever the running backend emits reaches `cb`.
 */
export function subscribeComposeProgress(
  cb: (p: ComposeProgress) => void,
): () => void {
  const onWindow = (e: Event) => {
    const detail = (e as CustomEvent).detail as ComposeProgress | undefined;
    if (detail) cb(detail);
  };
  window.addEventListener(COMPOSE_PROGRESS_EVENT, onWindow);

  let disposed = false;
  let unlistenTauri: (() => void) | undefined;
  if (isTauri()) {
    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<ComposeProgress>(COMPOSE_PROGRESS_EVENT, (e) => cb(e.payload)),
      )
      .then((un) => {
        if (disposed) un();
        else unlistenTauri = un;
      })
      .catch(() => {
        // No Tauri event bus (browser mock) — the window listener carries it.
      });
  }

  return () => {
    disposed = true;
    window.removeEventListener(COMPOSE_PROGRESS_EVENT, onWindow);
    unlistenTauri?.();
  };
}

/**
 * Render a fast preview proxy of the whole timeline to `output`. Backed by the
 * Rust `compose_preview_proxy` command. No-op resolving `false` off-Tauri so the
 * browser/demo degrades gracefully; resolves `true` when a proxy was produced.
 */
export async function renderPreviewProxy(
  project: Project,
  output: string,
): Promise<boolean> {
  if (!isTauri()) return false;
  await invoke("compose_preview_proxy", { project, output });
  return true;
}

/** H264 / CPU output settings derived from the project's own frame geometry. */
export function defaultComposeSettings(project: Project): ComposeSettings {
  const width = project.video_width > 0 ? project.video_width : 1920;
  const height = project.video_height > 0 ? project.video_height : 1080;
  const fps = project.video_fps > 0 ? Math.round(project.video_fps) : 30;
  return {
    width,
    height,
    fps,
    codec: "h264",
    encoder: "cpu",
    bitrate_kbps: null,
  };
}
