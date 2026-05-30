/**
 * First-run onboarding — Phase 9.1.
 *
 * Shown once (localStorage-gated in App) before the import screen: a welcome,
 * an optional "what do you make?" personalization, the Whisper model download
 * step (reusing the real downloader), then a hand-off to import or the demo.
 *
 * Kept deliberately skippable — nothing here blocks getting to work.
 */

import { useState } from "react";
import {
  Captions,
  ArrowRight,
  ArrowLeft,
  FileVideo,
  Sparkles,
  Upload,
  AlertTriangle,
} from "lucide-react";

import { ModelPicker } from "@/features/transcribe/ModelPicker";
import type { DownloadProgress, Project, WhisperModel } from "@/lib/bindings";
import { useT, type TKey } from "@/lib/i18n";
import { cn } from "@/lib/cn";
import { useVideoImport } from "@/lib/useVideoImport";

type Step = "welcome" | "profile" | "model" | "ready";

const PROFILES: Array<{ id: string; labelKey: TKey }> = [
  { id: "creator", labelKey: "obProfileCreator" },
  { id: "educator", labelKey: "obProfileEducator" },
  { id: "journalist", labelKey: "obProfileJournalist" },
  { id: "marketer", labelKey: "obProfileMarketer" },
  { id: "faith", labelKey: "obProfileFaith" },
  { id: "other", labelKey: "obProfileOther" },
];

interface Props {
  selected: WhisperModel | null;
  onSelect: (m: WhisperModel) => void;
  downloadedModels: WhisperModel[];
  downloading: { model: WhisperModel; progress: DownloadProgress } | null;
  onDownload: (m: WhisperModel) => void;
  onTryDemo: () => void;
  onImported: (project: Project) => void;
}

export function Onboarding({
  selected,
  onSelect,
  downloadedModels,
  downloading,
  onDownload,
  onTryDemo,
  onImported,
}: Props) {
  const t = useT();
  const [step, setStep] = useState<Step>("welcome");

  // Let the final step double as a drop target: drop a video here and we
  // import it straight away (and mark onboarding done, via onImported).
  const { dragging, busy, error, pickFile } = useVideoImport(onImported, {
    enabled: step === "ready",
  });

  function chooseProfile(id: string | null) {
    try {
      if (id) localStorage.setItem("sundayedit.profile", id);
    } catch {
      /* private mode / no storage — fine, it's only a default hint */
    }
    setStep("model");
  }

  return (
    <div className="grid h-screen w-screen place-items-center bg-[var(--color-bg)] p-8 text-[var(--color-fg)]">
      <div className="w-full max-w-xl">
        {step === "welcome" && (
          <div className="text-center">
            <div className="mx-auto mb-5 grid h-16 w-16 place-items-center rounded-2xl bg-[var(--color-accent-600)] text-[var(--color-neutral-950)]">
              <Captions size={30} />
            </div>
            <h1 className="text-[var(--text-ui-2xl)] font-semibold">
              {t("obWelcomeTitle")}
            </h1>
            <p className="mx-auto mt-3 max-w-md text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              {t("obWelcomeBody")}
            </p>
            <button
              type="button"
              onClick={() => setStep("profile")}
              className="mt-7 inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
            >
              {t("obGetStarted")} <ArrowRight size={15} />
            </button>
          </div>
        )}

        {step === "profile" && (
          <div className="text-center">
            <h2 className="text-[var(--text-ui-xl)] font-semibold">
              {t("obProfileTitle")}
            </h2>
            <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              {t("obProfileBody")}
            </p>
            <div className="mt-6 grid grid-cols-2 gap-2 sm:grid-cols-3">
              {PROFILES.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => chooseProfile(p.id)}
                  className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-4 text-[var(--text-ui-sm)] font-medium transition-colors hover:border-[var(--color-accent-500)] hover:bg-[var(--color-accent-500)]/8"
                >
                  {t(p.labelKey)}
                </button>
              ))}
            </div>
            <div className="mt-6 flex items-center justify-between">
              <button
                type="button"
                onClick={() => setStep("welcome")}
                className="inline-flex items-center gap-1.5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
              >
                <ArrowLeft size={14} /> {t("obBack")}
              </button>
              <button
                type="button"
                onClick={() => chooseProfile(null)}
                className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] underline-offset-4 hover:text-[var(--color-accent-400)] hover:underline"
              >
                {t("obSkip")}
              </button>
            </div>
          </div>
        )}

        {step === "model" && (
          <div>
            <ModelPicker
              selected={selected}
              onSelect={onSelect}
              downloadedModels={downloadedModels}
              downloading={downloading}
              onDownload={onDownload}
            />
            <div className="mt-6 flex items-center justify-between">
              <button
                type="button"
                onClick={() => setStep("profile")}
                className="inline-flex items-center gap-1.5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
              >
                <ArrowLeft size={14} /> {t("obBack")}
              </button>
              <button
                type="button"
                onClick={() => setStep("ready")}
                className="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
              >
                {t("obContinue")} <ArrowRight size={15} />
              </button>
            </div>
          </div>
        )}

        {step === "ready" && (
          <div className="text-center">
            <div
              className={cn(
                "rounded-2xl border-2 border-dashed px-8 py-12 transition-colors",
                dragging
                  ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/5"
                  : "border-[var(--color-border-strong)]",
              )}
            >
              <div className="mx-auto mb-5 grid h-14 w-14 place-items-center rounded-2xl bg-[var(--color-bg-surface)] text-[var(--color-accent-400)]">
                {busy ? (
                  <Upload size={26} className="animate-pulse" />
                ) : (
                  <Sparkles size={26} />
                )}
              </div>
              <h2 className="text-[var(--text-ui-xl)] font-semibold">
                {busy ? t("importReading") : t("obReadyTitle")}
              </h2>
              <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
                {t("obReadyBody")}
              </p>
              <div className="mt-7 flex items-center justify-center gap-3">
                <button
                  type="button"
                  onClick={pickFile}
                  disabled={busy}
                  className="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-50"
                >
                  <FileVideo size={15} /> {t("obImportVideo")}
                </button>
                <button
                  type="button"
                  onClick={onTryDemo}
                  disabled={busy}
                  className="rounded-lg border border-[var(--color-border)] px-5 py-2.5 text-[var(--text-ui-sm)] font-medium hover:border-[var(--color-accent-600)] disabled:opacity-50"
                >
                  {t("obExploreDemo")}
                </button>
              </div>
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
        )}
      </div>
    </div>
  );
}
