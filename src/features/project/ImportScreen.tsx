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

import { FileVideo, Upload, AlertTriangle } from "lucide-react";

import type { Project } from "@/lib/bindings";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";
import { useVideoImport } from "@/lib/useVideoImport";

interface Props {
  onProjectReady: (project: Project) => void;
}

export function ImportScreen({ onProjectReady }: Props) {
  const t = useT();
  const { dragging, busy, error, pickFile } = useVideoImport(onProjectReady);

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
            {busy ? t("importReading") : t("importDropHere")}
          </h1>
          <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            {t("importFormats")}
            <br />
            {t("importNeverLeaves")}
          </p>
          <button
            type="button"
            onClick={pickFile}
            disabled={busy}
            className="mt-6 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] transition-colors hover:bg-[var(--color-accent-500)] disabled:opacity-50"
          >
            {t("importPickFile")}
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
