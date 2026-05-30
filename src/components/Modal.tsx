/**
 * Modal overlay — used for the pipeline/output/config operations that don't
 * belong as a persistent panel beside the editor (Transcribe, AI clips,
 * Export, Settings). Backdrop click, Escape, and the ✕ all close it.
 */

import { useEffect, type ReactNode } from "react";
import { X } from "lucide-react";

import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";

interface Props {
  title: string;
  onClose: () => void;
  children: ReactNode;
  /** Tailwind max-width class for the panel. Defaults to a roomy 3xl. */
  widthClass?: string;
}

export function Modal({
  title,
  onClose,
  children,
  widthClass = "max-w-3xl",
}: Props) {
  const t = useT();

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-[var(--color-neutral-950)]/70 p-6 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className={cn(
          "my-6 flex max-h-[88vh] w-full flex-col overflow-hidden rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-2xl",
          widthClass,
        )}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-[var(--color-border)] px-5 py-3">
          <h2 className="text-[var(--text-ui-lg)] font-semibold">{title}</h2>
          <button
            type="button"
            onClick={onClose}
            title={t("actionClose")}
            aria-label={t("actionClose")}
            className="rounded-md p-1.5 text-[var(--color-fg-subtle)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)]"
          >
            <X size={18} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      </div>
    </div>
  );
}
