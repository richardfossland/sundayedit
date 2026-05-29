/**
 * Model picker — Phase 2.1 first-run choice.
 *
 * Shows the Whisper model catalog with size/speed tradeoffs. The recommended
 * model (large-v3-turbo) is highlighted. A model that isn't on disk yet shows
 * a "Last ned" action that fetches it from Hugging Face (asr_download_model)
 * with a live progress bar; the catalog itself is data-driven from the Rust
 * registry.
 */

import { useQuery } from "@tanstack/react-query";
import { Check, Download, Cpu, Star, X } from "lucide-react";

import { ipc } from "@/lib/ipc";
import type { DownloadProgress, WhisperModel } from "@/lib/bindings";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";

interface Props {
  selected: WhisperModel | null;
  onSelect: (model: WhisperModel) => void;
  downloadedModels?: WhisperModel[];
  downloading?: { model: WhisperModel; progress: DownloadProgress } | null;
  onDownload?: (model: WhisperModel) => void;
}

export function ModelPicker({
  selected,
  onSelect,
  downloadedModels = [],
  downloading = null,
  onDownload,
}: Props) {
  const t = useT();
  const modelsQuery = useQuery({
    queryKey: ["asr-models"],
    queryFn: () => ipc.asr.listModels(),
  });

  const models = modelsQuery.data ?? [];

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-5 flex items-center gap-2">
        <Cpu size={18} className="text-[var(--color-accent-400)]" />
        <h2 className="text-[var(--text-ui-lg)] font-semibold">
          {t("modelTitle")}
        </h2>
      </div>
      <p className="mb-5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        {t("modelIntro")}
      </p>

      <ul className="space-y-2">
        {models.map((m) => {
          const isSelected = selected === m.model;
          const isDownloaded = downloadedModels.includes(m.model);
          const isDownloading = downloading?.model === m.model;
          return (
            <li key={m.model} className="flex items-stretch gap-2">
              <button
                type="button"
                onClick={() => onSelect(m.model)}
                className={cn(
                  "flex flex-1 items-start gap-3 rounded-lg border px-4 py-3 text-left transition-colors",
                  isSelected
                    ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/8"
                    : "border-[var(--color-border)] hover:border-[var(--color-border-strong)]",
                )}
              >
                <div
                  className={cn(
                    "mt-0.5 grid h-5 w-5 shrink-0 place-items-center rounded-full border",
                    isSelected
                      ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)] text-[var(--color-neutral-950)]"
                      : "border-[var(--color-border-strong)]",
                  )}
                >
                  {isSelected && <Check size={12} />}
                </div>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-[var(--text-ui-sm)] font-semibold">
                      {m.filename.replace("ggml-", "").replace(".bin", "")}
                    </span>
                    {m.recommended && (
                      <span className="flex items-center gap-1 rounded-full bg-[var(--color-accent-600)]/20 px-2 py-0.5 text-[10px] font-medium text-[var(--color-accent-300)]">
                        <Star size={9} fill="currentColor" />{" "}
                        {t("modelRecommended")}
                      </span>
                    )}
                    {isDownloaded ? (
                      <span className="flex items-center gap-1 text-[10px] text-[var(--color-success)]">
                        <Check size={10} /> {t("modelDownloaded")}
                      </span>
                    ) : (
                      <span className="flex items-center gap-1 text-[10px] text-[var(--color-fg-subtle)]">
                        <Download size={10} /> {m.approx_mb} MB
                      </span>
                    )}
                  </div>
                  <p className="mt-0.5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
                    {m.description}
                  </p>
                </div>
              </button>

              {!isDownloaded && (
                <div className="flex w-32 shrink-0 items-center justify-center rounded-lg border border-[var(--color-border)] px-2">
                  {isDownloading ? (
                    <DownloadStatus progress={downloading.progress} />
                  ) : (
                    <button
                      type="button"
                      onClick={() => onDownload?.(m.model)}
                      disabled={!onDownload || !!downloading}
                      className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent-600)] px-3 py-1.5 text-[var(--text-ui-xs)] font-semibold text-[var(--color-neutral-950)] transition-colors hover:bg-[var(--color-accent-500)] disabled:opacity-40"
                    >
                      <Download size={12} /> {t("modelDownload")}
                    </button>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function DownloadStatus({ progress }: { progress: DownloadProgress }) {
  const t = useT();
  const pct =
    progress.fraction != null ? Math.round(progress.fraction * 100) : null;
  return (
    <div className="w-full">
      <div className="mb-1 flex items-center justify-between text-[10px] text-[var(--color-fg-muted)]">
        <span>{pct != null ? `${pct}%` : t("modelDownloading")}</span>
        <button
          type="button"
          onClick={() => void ipc.asr.cancelDownload()}
          title={t("actionCancel")}
          aria-label={t("modelCancelDownload")}
          className="opacity-70 hover:opacity-100"
        >
          <X size={12} />
        </button>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-[var(--color-bg-surface)]">
        <div
          className="h-full bg-[var(--color-accent-500)] transition-[width]"
          style={{ width: pct != null ? `${pct}%` : "40%" }}
        />
      </div>
    </div>
  );
}
