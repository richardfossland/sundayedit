/**
 * Model picker — Phase 2.1 first-run choice.
 *
 * Shows the Whisper model catalog with size/speed tradeoffs. The
 * recommended model (large-v3-turbo) is highlighted. Selecting a model
 * that isn't downloaded yet would trigger a download (wired to the
 * download command when the `whisper` feature build ships); for now this
 * is the catalog UI, which is real and data-driven from the Rust registry.
 */

import { useQuery } from "@tanstack/react-query";
import { Check, Download, Cpu, Star } from "lucide-react";

import { ipc } from "@/lib/ipc";
import type { WhisperModel } from "@/lib/bindings";
import { cn } from "@/lib/cn";

interface Props {
  selected: WhisperModel | null;
  onSelect: (model: WhisperModel) => void;
  downloadedModels?: WhisperModel[];
}

export function ModelPicker({
  selected,
  onSelect,
  downloadedModels = [],
}: Props) {
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
          Velg transkripsjonsmodell
        </h2>
      </div>
      <p className="mb-5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        Kjører helt lokalt på maskinen din — ingenting lastes opp. Større
        modeller er mer nøyaktige men tregere.
      </p>

      <ul className="space-y-2">
        {models.map((m) => {
          const isSelected = selected === m.model;
          const isDownloaded = downloadedModels.includes(m.model);
          return (
            <li key={m.model}>
              <button
                type="button"
                onClick={() => onSelect(m.model)}
                className={cn(
                  "flex w-full items-start gap-3 rounded-lg border px-4 py-3 text-left transition-colors",
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
                        <Star size={9} fill="currentColor" /> Anbefalt
                      </span>
                    )}
                    {isDownloaded ? (
                      <span className="flex items-center gap-1 text-[10px] text-[var(--color-success)]">
                        <Check size={10} /> Lastet ned
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
            </li>
          );
        })}
      </ul>
    </div>
  );
}
