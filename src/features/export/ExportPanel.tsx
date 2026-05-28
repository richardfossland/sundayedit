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
import { cn } from "@/lib/cn";

interface Props {
  project: Project;
}

type SidecarFormat = "srt" | "vtt" | "ass" | "txt";

export function ExportPanel({ project }: Props) {
  const [exported, setExported] = useState<{ format: string; content: string } | null>(null);

  const presetsQuery = useQuery({
    queryKey: ["export-presets"],
    queryFn: () => ipc.render.listExportPresets(),
  });
  const presets = presetsQuery.data ?? [];

  const [selectedPreset, setSelectedPreset] = useState<ExportPreset | null>(null);
  const [warnings, setWarnings] = useState<ExportWarning[]>([]);
  const [rendering, setRendering] = useState(false);
  const [renderResult, setRenderResult] = useState<string | null>(null);

  // Validate whenever the chosen platform changes.
  useEffect(() => {
    if (!selectedPreset) { setWarnings([]); return; }
    let cancelled = false;
    ipc.render.validate(project, selectedPreset)
      .then((w) => { if (!cancelled) setWarnings(w); })
      .catch(() => { if (!cancelled) setWarnings([]); });
    return () => { cancelled = true; };
  }, [selectedPreset, project]);

  async function doSidecar(format: SidecarFormat) {
    const content =
      format === "srt" ? await ipc.exporters.srt(project, true)
      : format === "vtt" ? await ipc.exporters.vtt(project, true)
      : format === "ass" ? await ipc.exporters.ass(project)
      : await ipc.exporters.txt(project, true);
    setExported({ format, content });
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
      setRenderResult(`Ferdig: ${out}`);
    } catch (e) {
      setRenderResult(
        e instanceof IPCError ? `Feil: ${e.message}` : `Feil: ${String(e)}`,
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
          Tekstformater (sidecar)
        </h3>
        {([
          { id: "srt", label: "SRT", desc: "Universal — YouTube, de fleste spillere" },
          { id: "vtt", label: "VTT", desc: "Web-standard, med talere" },
          { id: "ass", label: "ASS", desc: "Full styling — Aegisub, burn-in" },
          { id: "txt", label: "TXT", desc: "Ren transkripsjon" },
        ] as Array<{ id: SidecarFormat; label: string; desc: string }>).map((f) => (
          <button
            key={f.id}
            type="button"
            onClick={() => doSidecar(f.id)}
            className="flex w-full flex-col rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-2 text-left transition-colors hover:border-[var(--color-accent-600)]"
          >
            <span className="font-mono text-[var(--text-ui-sm)] font-semibold text-[var(--color-accent-400)]">{f.label}</span>
            <span className="text-[10px] text-[var(--color-fg-muted)]">{f.desc}</span>
          </button>
        ))}
      </div>

      {/* Platform burn-in */}
      <div className="w-72 shrink-0 space-y-2 overflow-y-auto border-r border-[var(--color-border)] p-4">
        <h3 className="mb-1 flex items-center gap-1.5 text-[var(--text-ui-xs)] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
          <Film size={12} /> Brenn inn (plattform)
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
            <span className="text-[var(--text-ui-sm)] font-medium">{p.name}</span>
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
            <h2 className="text-[var(--text-ui-lg)] font-semibold">{selectedPreset.name}</h2>
            <p className="mt-1 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              {selectedPreset.width}×{selectedPreset.height}
              {selectedPreset.max_duration_sec ? ` · maks ${selectedPreset.max_duration_sec}s` : ""}
              {selectedPreset.also_srt_sidecar ? " · + SRT-sidecar" : ""}
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
                        w.severity === "error" ? "text-[var(--color-danger)]" : "text-[var(--color-warning)]",
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
              {rendering ? <Loader2 size={15} className="animate-spin" /> : <Download size={15} />}
              {rendering ? "Brenner inn…" : "Brenn inn undertekster"}
            </button>

            {renderResult && (
              <p className="mt-3 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">{renderResult}</p>
            )}
          </div>
        ) : exported ? (
          <pre className="whitespace-pre-wrap rounded-md bg-[var(--color-bg-elevated)] p-4 font-mono text-[var(--text-ui-xs)] leading-relaxed">
            {exported.content}
          </pre>
        ) : (
          <div className="grid h-full place-items-center text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            Velg et tekstformat eller en plattform til venstre.
          </div>
        )}
      </div>
    </div>
  );
}
