/**
 * Shared video-import behaviour — Phase 1.1.
 *
 * Tauri's window-level drag-drop delivers absolute file paths (the HTML5 drop
 * event only gives sandboxed File handles), so any screen that wants "drop a
 * video to start" subscribes to the same webview event. Both the dedicated
 * import screen and the onboarding hand-off reuse this hook, so drop + pick +
 * error handling stay identical everywhere.
 *
 * `enabled` gates the drop listener — a screen can mount the hook but only
 * accept drops on the step where it makes sense.
 */

import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { ipc, IPCError, project as projectApi } from "@/lib/ipc";
import type { Project } from "@/lib/bindings";
import { useT } from "@/lib/i18n";

const FALLBACK_EXTS = [
  "mp4",
  "mov",
  "mkv",
  "webm",
  "avi",
  "m4v",
  "mp3",
  "wav",
  "m4a",
  "flac",
  "ogg",
];

export function useVideoImport(
  onReady: (project: Project) => void,
  options?: { enabled?: boolean },
) {
  const enabled = options?.enabled ?? true;
  const t = useT();
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function importPath(path: string) {
    setBusy(true);
    setError(null);
    try {
      const proj = await ipc.project.createFromVideo(path);
      onReady(proj);
    } catch (e) {
      if (e instanceof IPCError) {
        setError(
          e.code === "video_missing"
            ? t("importFileMissing")
            : e.code === "validation"
              ? t("importReadError", { error: e.message })
              : e.message,
        );
      } else {
        setError(String(e));
      }
    } finally {
      setBusy(false);
    }
  }

  // Tauri window-level drag-drop gives us real file paths.
  useEffect(() => {
    if (!enabled) {
      setDragging(false);
      return;
    }
    let unlisten: (() => void) | undefined;
    let webview: ReturnType<typeof getCurrentWebview>;
    try {
      // Throws synchronously outside Tauri (browser dev/preview); the picker
      // and demo still work, so degrade quietly.
      webview = getCurrentWebview();
    } catch {
      return;
    }
    webview
      .onDragDropEvent((event) => {
        if (event.payload.type === "over" || event.payload.type === "enter") {
          setDragging(true);
        } else if (event.payload.type === "drop") {
          setDragging(false);
          const path = event.payload.paths[0];
          if (path) void importPath(path);
        } else {
          setDragging(false);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* not in Tauri (browser dev) — picker still works */
      });
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  async function pickFile() {
    const exts = await projectApi
      .acceptedExtensions()
      .catch(() => FALLBACK_EXTS);
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: t("importFilterName"), extensions: exts }],
    });
    if (typeof selected === "string") void importPath(selected);
  }

  return { dragging, busy, error, pickFile, importPath };
}
