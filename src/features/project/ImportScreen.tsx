/**
 * Import screen — Phase 1.1 entry point.
 *
 * Drag a video onto the window (or click to pick) → we probe metadata →
 * create a project. In dev without ffmpeg installed, probing surfaces a
 * clear error rather than crashing.
 *
 * Tauri's drag-drop delivers absolute file paths via the window event;
 * the HTML5 drop event only gives sandboxed File handles, so we read the
 * Tauri event payload.
 */

import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FileVideo, Upload, AlertTriangle } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import { project as projectApi } from "@/lib/ipc";
import type { Project } from "@/lib/bindings";
import { cn } from "@/lib/cn";

interface Props {
  onProjectReady: (project: Project) => void;
}

export function ImportScreen({ onProjectReady }: Props) {
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Tauri window-level drag-drop gives us real file paths.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let webview: ReturnType<typeof getCurrentWebview>;
    try {
      // Throws synchronously outside Tauri (browser dev/preview); the demo
      // and file picker still work, so degrade quietly.
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
  }, []);

  async function pickFile() {
    const exts = await projectApi
      .acceptedExtensions()
      .catch(() => [
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
      ]);
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "Video & lyd", extensions: exts }],
    });
    if (typeof selected === "string") void importPath(selected);
  }

  async function importPath(path: string) {
    setBusy(true);
    setError(null);
    try {
      const proj = await ipc.project.createFromVideo(path);
      onProjectReady(proj);
    } catch (e) {
      if (e instanceof IPCError) {
        setError(
          e.code === "video_missing"
            ? "Filen finnes ikke lenger."
            : e.code === "validation"
              ? `Kunne ikke lese filen: ${e.message}`
              : e.message,
        );
      } else {
        setError(String(e));
      }
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid h-full place-items-center p-8">
      <div className="w-full max-w-xl text-center">
        <div
          className={cn(
            "rounded-2xl border-2 border-dashed p-16 transition-colors",
            dragging
              ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/5"
              : "border-[var(--color-border-strong)]",
          )}
        >
          <div className="mx-auto mb-5 grid h-16 w-16 place-items-center rounded-2xl bg-[var(--color-bg-surface)]">
            {busy ? (
              <Upload
                size={28}
                className="animate-pulse text-[var(--color-accent-400)]"
              />
            ) : (
              <FileVideo size={28} className="text-[var(--color-fg-muted)]" />
            )}
          </div>
          <h1 className="text-[var(--text-ui-xl)] font-semibold">
            {busy ? "Leser video…" : "Slipp en video her"}
          </h1>
          <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            MP4, MOV, MKV, WebM, AVI — eller lyd: MP3, WAV, M4A, FLAC, OGG.
            <br />
            Filen forlater aldri maskinen din.
          </p>
          <button
            type="button"
            onClick={pickFile}
            disabled={busy}
            className="mt-6 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] transition-colors hover:bg-[var(--color-accent-500)] disabled:opacity-50"
          >
            Velg fil…
          </button>
        </div>

        {error && (
          <div className="mt-4 flex items-start gap-2 rounded-md border border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10 px-4 py-3 text-left text-[var(--text-ui-sm)]">
            <AlertTriangle
              size={16}
              className="mt-0.5 shrink-0 text-[var(--color-danger)]"
            />
            <span>{error}</span>
          </div>
        )}
      </div>
    </div>
  );
}
