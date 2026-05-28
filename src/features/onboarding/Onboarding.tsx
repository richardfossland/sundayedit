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
} from "lucide-react";

import { ModelPicker } from "@/features/transcribe/ModelPicker";
import type { DownloadProgress, WhisperModel } from "@/lib/bindings";

type Step = "welcome" | "profile" | "model" | "ready";

const PROFILES: Array<{ id: string; label: string }> = [
  { id: "creator", label: "Innholdsskaper" },
  { id: "educator", label: "Underviser" },
  { id: "journalist", label: "Journalist" },
  { id: "marketer", label: "Markedsfører" },
  { id: "faith", label: "Menighet / kirke" },
  { id: "other", label: "Annet" },
];

interface Props {
  selected: WhisperModel | null;
  onSelect: (m: WhisperModel) => void;
  downloadedModels: WhisperModel[];
  downloading: { model: WhisperModel; progress: DownloadProgress } | null;
  onDownload: (m: WhisperModel) => void;
  onDone: () => void;
  onTryDemo: () => void;
}

export function Onboarding({
  selected,
  onSelect,
  downloadedModels,
  downloading,
  onDownload,
  onDone,
  onTryDemo,
}: Props) {
  const [step, setStep] = useState<Step>("welcome");

  function chooseProfile(id: string | null) {
    try {
      if (id) localStorage.setItem("verbatim.profile", id);
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
              Velkommen til Verbatim
            </h1>
            <p className="mx-auto mt-3 max-w-md text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              Fra rå video til kringkastingsklar teksting — raskt. AI gjør 92 %
              av jobben og viser deg nøyaktig hvor de siste 8 % trenger et
              blikk. Alt kjører lokalt; videoen forlater aldri maskinen din.
            </p>
            <button
              type="button"
              onClick={() => setStep("profile")}
              className="mt-7 inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
            >
              Kom i gang <ArrowRight size={15} />
            </button>
          </div>
        )}

        {step === "profile" && (
          <div className="text-center">
            <h2 className="text-[var(--text-ui-xl)] font-semibold">
              Hva slags innhold lager du?
            </h2>
            <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              Hjelper oss å foreslå fornuftige standarder. Helt valgfritt.
            </p>
            <div className="mt-6 grid grid-cols-2 gap-2 sm:grid-cols-3">
              {PROFILES.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => chooseProfile(p.id)}
                  className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-4 text-[var(--text-ui-sm)] font-medium transition-colors hover:border-[var(--color-accent-500)] hover:bg-[var(--color-accent-500)]/8"
                >
                  {p.label}
                </button>
              ))}
            </div>
            <div className="mt-6 flex items-center justify-between">
              <button
                type="button"
                onClick={() => setStep("welcome")}
                className="inline-flex items-center gap-1.5 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]"
              >
                <ArrowLeft size={14} /> Tilbake
              </button>
              <button
                type="button"
                onClick={() => chooseProfile(null)}
                className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] underline-offset-4 hover:text-[var(--color-accent-400)] hover:underline"
              >
                Hopp over
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
                <ArrowLeft size={14} /> Tilbake
              </button>
              <button
                type="button"
                onClick={() => setStep("ready")}
                className="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
              >
                Fortsett <ArrowRight size={15} />
              </button>
            </div>
          </div>
        )}

        {step === "ready" && (
          <div className="text-center">
            <div className="mx-auto mb-5 grid h-14 w-14 place-items-center rounded-2xl bg-[var(--color-bg-surface)] text-[var(--color-accent-400)]">
              <Sparkles size={26} />
            </div>
            <h2 className="text-[var(--text-ui-xl)] font-semibold">Klar!</h2>
            <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              Slipp inn en video for å starte — eller utforsk demo-prosjektet
              først.
            </p>
            <div className="mt-7 flex items-center justify-center gap-3">
              <button
                type="button"
                onClick={onDone}
                className="inline-flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-5 py-2.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
              >
                <FileVideo size={15} /> Importer en video
              </button>
              <button
                type="button"
                onClick={onTryDemo}
                className="rounded-lg border border-[var(--color-border)] px-5 py-2.5 text-[var(--text-ui-sm)] font-medium hover:border-[var(--color-accent-600)]"
              >
                Utforsk demo-prosjektet
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
