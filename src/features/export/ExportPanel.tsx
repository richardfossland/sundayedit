/**
 * Export panel — Phase 6.1 (sidecar formats) + 6.2/6.3 (burn-in +
 * platform presets).
 *
 * Two columns:
 *   - Left: sidecar text formats (SRT/VTT/ASS/TXT) — instant, in-memory.
 *   - Right: platform burn-in. Pick a platform → we validate (duration,
 *     aspect, captions) and show warnings BEFORE the long render, then
 *     "Burn in" writes a captioned MP4. Without ffmpeg installed the
 *     command errors clearly (we surface it).
 */

import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Film, Download, Loader2 } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { ExportPreset, ExportWarning, Project } from "@/lib/bindings";
import { useT, type TKey } from "@/lib/i18n";
import { cn } from "@/lib/cn";

interface Props {
  project: Project;
}

type SidecarFormat = "srt" | "vtt" | "ass" | "txt" | "json";

export function ExportPanel({ project }: Props) {
  const t = useT();
  const [exported, setExported] = useState<{
    format: string;
    content: string;
  } | null>(null);

  const presetsQuery = useQuery({
    queryKey: ["export-presets"],
    queryFn: () => ipc.render.listExportPresets(),
  });
  const presets = presetsQuery.data ?? [];

  const [selectedPreset, setSelectedPreset] = useState<ExportPreset | null>(
    null,
  );
  const [warnings, setWarnings] = useState<ExportWarning[]>([]);
  const [rendering, setRendering] = useState(false);
  const [renderResult, setRenderResult] = useState<string | null>(null);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);

  // Validate whenever the chosen platform changes.
  useEffect(() => {
    if (!selectedPreset) {
      setWarnings([]);
      return;
    }
    let cancelled = false;
    ipc.render
      .validate(project, selectedPreset)
      .then((w) => {
        if (!cancelled) setWarnings(w);
      })
      .catch(() => {
        if (!cancelled) setWarnings([]);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedPreset, project]);

  async function doSidecar(format: SidecarFormat) {
    const content =
      format === "srt"
        ? await ipc.exporters.srt(project, true)
        : format === "vtt"
          ? await ipc.exporters.vtt(project, true)
          : format === "ass"
            ? await ipc.exporters.ass(project)
            : format === "json"
              ? await ipc.exporters.json(project, true)
              : await ipc.exporters.txt(project, true);
    setExported({ format, content });
  }

  // DOCX is binary → save straight to disk; text formats preview first.
  function handleFormat(format: SidecarFormat | "docx") {
    if (format === "docx") {
      void doSaveExport("docx");
      return;
    }
    void doSidecar(format);
  }

  async function doSaveExport(format: string) {
    const base = project.name.replace(/\.[^.]+$/, "");
    const out = await saveDialog({
      defaultPath: `${base}.${format}`,
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (typeof out !== "string") return;
    setSaveMsg(null);
    try {
      await ipc.exporters.save(project, out, format);
      setSaveMsg(t("doneFile", { path: out }));
    } catch (e) {
      setSaveMsg(
        e instanceof IPCError
          ? t("errorPrefix", { error: e.message })
          : String(e),
      );
    }
  }

  async function doBurnIn() {
    if (!selectedPreset) return;
    if (warnings.some((w) => w.severity === "error")) return;
    const out = await saveDialog({
      defaultPath: `${project.name.replace(/\.[^.]+$/, "")}_captioned.mp4`,
      filters: [{ name: "Video", extensions: ["mp4"] }],
    });
    if (typeof out !== "string") return;

    setRendering(true);
    setRenderResult(null);
    try {
      await ipc.render.burnInPreset(project, out, selectedPreset);
      setRenderResult(t("doneFile", { path: out }));
    } catch (e) {
      setRenderResult(
        e instanceof IPCError
          ? t("errorPrefix", { error: e.message })
          : t("errorPrefix", { error: String(e) }),
      );
    } finally {
      setRendering(false);
    }
  }

  const hasBlockingError = warnings.some((w) => w.severity === "error");

  return (
    <div className="flex h-full overflow-hidden">
      {/* Sidecar formats */}
      <div className="w-72 shrink-0 space-y-2 overflow-y-auto border-r border-[var(--color-border)] p-4">
        <h3 className="mb-1 text-[var(--text-ui-xs)] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
          {t("exportSidecarHeader")}
        </h3>
        {(
          [
            { id: "srt", label: "SRT", descKey: "exportSrtDesc" },
            { id: "vtt", label: "VTT", descKey: "exportVttDesc" },
            { id: "ass", label: "ASS", descKey: "exportAssDesc" },
            { id: "txt", label: "TXT", descKey: "exportTxtDesc" },
            { id: "json", label: "JSON", descKey: "exportJsonDesc" },
            { id: "docx", label: "DOCX", descKey: "exportDocxDesc" },
          ] as Array<{
            id: SidecarFormat | "docx";
            label: string;
            descKey: TKey;
          }>
        ).map((f) => (
          <button
            key={f.id}
            type="button"
            onClick={() => handleFormat(f.id)}
            className="flex w-full flex-col rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-2 text-left transition-colors hover:border-[var(--color-accent-600)]"
          >
            <span className="font-mono text-[var(--text-ui-sm)] font-semibold text-[var(--color-accent-400)]">
              {f.label}
            </span>
            <span className="text-[10px] text-[var(--color-fg-muted)]">
              {t(f.descKey)}
            </span>
          </button>
        ))}
      </div>

      {/* Platform burn-in */}
      <div className="w-72 shrink-0 space-y-2 overflow-y-auto border-r border-[var(--color-border)] p-4">
        <h3 className="mb-1 flex items-center gap-1.5 text-[var(--text-ui-xs)] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
          <Film size={12} /> {t("exportPlatformHeader")}
        </h3>
        {presets.map((p) => (
          <button
            key={p.id}
            type="button"
            onClick={() => setSelectedPreset(p)}
            className={cn(
              "flex w-full flex-col rounded-md border px-3 py-2 text-left transition-colors",
              selectedPreset?.id === p.id
                ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/8"
                : "border-[var(--color-border)] hover:border-[var(--color-border-strong)]",
            )}
          >
            <span className="text-[var(--text-ui-sm)] font-medium">
              {p.name}
            </span>
            <span className="text-[10px] text-[var(--color-fg-muted)]">
              {p.width}×{p.height} · {p.description}
            </span>
          </button>
        ))}
      </div>

      {/* Detail / preview */}
      <div className="flex-1 overflow-auto p-4">
        {selectedPreset ? (
          <div className="max-w-xl">
            <h2 className="text-[var(--text-ui-lg)] font-semibold">
              {selectedPreset.name}
            </h2>
            <p className="mt-1 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              {selectedPreset.width}×{selectedPreset.height}
              {selectedPreset.max_duration_sec
                ? ` · ${t("exportMaxDuration", { n: String(selectedPreset.max_duration_sec) })}`
                : ""}
              {selectedPreset.also_srt_sidecar
                ? ` · ${t("exportSrtSidecar")}`
                : ""}
            </p>

            {warnings.length > 0 && (
              <ul className="mt-4 space-y-2">
                {warnings.map((w, i) => (
                  <li
                    key={i}
                    className={cn(
                      "flex items-start gap-2 rounded-md border px-3 py-2 text-[var(--text-ui-sm)]",
                      w.severity === "error"
                        ? "border-[var(--color-danger)]/40 bg-[var(--color-danger)]/10"
                        : "border-[var(--color-warning)]/40 bg-[var(--color-warning)]/10",
                    )}
                  >
                    <AlertTriangle
                      size={15}
                      className={cn(
                        "mt-0.5 shrink-0",
                        w.severity === "error"
                          ? "text-[var(--color-danger)]"
                          : "text-[var(--color-warning)]",
                      )}
                    />
                    <span>{w.message}</span>
                  </li>
                ))}
              </ul>
            )}

            <button
              type="button"
              onClick={doBurnIn}
              disabled={rendering || hasBlockingError}
              className="mt-5 flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-50"
            >
              {rendering ? (
                <Loader2 size={15} className="animate-spin" />
              ) : (
                <Download size={15} />
              )}
              {rendering ? t("exportBurningIn") : t("exportBurnIn")}
            </button>

            {renderResult && (
              <p className="mt-3 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
                {renderResult}
              </p>
            )}
          </div>
        ) : exported ? (
          <div className="flex h-full flex-col">
            <div className="mb-3 flex items-center gap-3">
              <button
                type="button"
                onClick={() => doSaveExport(exported.format)}
                className="flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-4 py-2 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
              >
                <Download size={14} />{" "}
                {t("exportSaveFormat", {
                  format: exported.format.toUpperCase(),
                })}
              </button>
              {saveMsg && (
                <span className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
                  {saveMsg}
                </span>
              )}
            </div>
            <pre className="flex-1 overflow-auto whitespace-pre-wrap rounded-md bg-[var(--color-bg-elevated)] p-4 font-mono text-[var(--text-ui-xs)] leading-relaxed">
              {exported.content}
            </pre>
          </div>
        ) : (
          <div className="grid h-full place-items-center text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            {t("exportChooseHint")}
          </div>
        )}
      </div>
    </div>
  );
}
