/**
 * Local transcription — the default, privacy-first path (Phase 2.1 + 3.4).
 *
 * Local Whisper keeps the video on the user's machine; this is the panel that
 * actually runs it. It needs a downloaded model (picked above) and a 16 kHz
 * mono WAV — which it extracts from the source media on first use and caches on
 * the project (`audio_wav_path`, also reused by diarization). Progress streams
 * from the backend via `transcribe-progress`. On a build without the `whisper`
 * feature the backend errors clearly and we surface that here.
 */

import { useRef, useState } from "react";
import { appCacheDir, appDataDir, join } from "@tauri-apps/api/path";
import { Cpu, Loader2, ShieldCheck, Download, X } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { Caption, Project, WhisperModel } from "@/lib/bindings";
import { useT } from "@/lib/i18n";
import { buildAsrOptions } from "./asrOptions";

interface Props {
  project: Project;
  /** The model selected in the picker above. */
  model: WhisperModel | null;
  /** Models already on disk — the button needs the selected one present. */
  downloadedModels: WhisperModel[];
  onTranscribed: (captions: Caption[]) => void;
  /** Lift the extracted WAV path back onto the project so later steps reuse it. */
  onProjectChange: (project: Project) => void;
}

type Phase =
  | { stage: "idle" }
  | { stage: "extracting" }
  | { stage: "preparing" }
  | { stage: "running"; fraction: number };

export function LocalPanel({
  project,
  model,
  downloadedModels,
  onTranscribed,
  onProjectChange,
}: Props) {
  const t = useT();
  const [phase, setPhase] = useState<Phase>({ stage: "idle" });
  const [error, setError] = useState<string | null>(null);
  // Keep the latest project in a ref so the long-running async transcribe reads
  // a fresh copy (audio_wav_path may have just been set) without re-binding.
  const projectRef = useRef(project);
  projectRef.current = project;

  const modelReady = model != null && downloadedModels.includes(model);
  const busy = phase.stage !== "idle";

  async function doTranscribe() {
    if (model == null || busy) return;
    setError(null);
    let unlisten: (() => void) | undefined;
    try {
      // 1. Ensure we have a 16 kHz mono WAV. Reuse the project's if it's still
      //    on disk; otherwise extract once and remember it on the project.
      setPhase({ stage: "extracting" });
      let wav = projectRef.current.audio_wav_path;
      if (!wav) {
        const cacheDir = await appCacheDir();
        wav = await ipc.project.extractAudio(
          projectRef.current.video_path,
          cacheDir,
        );
        const withWav = { ...projectRef.current, audio_wav_path: wav };
        onProjectChange(withWav);
        projectRef.current = withWav;
      }

      // 2. Stream progress while the model runs.
      setPhase({ stage: "preparing" });
      // Inference ("running", whisper's own callback) covers 0–90% of the bar;
      // the post-inference segment stream covers the last 10%. Monotonic via
      // max() so the hand-off between the two phases can't jump backwards.
      unlisten = await ipc.asr.onTranscribeProgress((p) => {
        if (p.kind === "preparing") setPhase({ stage: "preparing" });
        else if (p.kind === "running" || p.kind === "segment") {
          const fraction =
            p.kind === "running" ? p.fraction * 0.9 : 0.9 + p.fraction * 0.1;
          setPhase((prev) => ({
            stage: "running",
            fraction:
              prev.stage === "running"
                ? Math.max(prev.fraction, fraction)
                : fraction,
          }));
        }
      });

      const modelsDir = await join(await appDataDir(), "models");
      const captions = await ipc.asr.transcribeLocal(
        wav,
        modelsDir,
        model,
        buildAsrOptions(projectRef.current),
      );
      onTranscribed(captions);
    } catch (e) {
      const msg = e instanceof IPCError ? e.message : String(e);
      // A user-initiated cancel is not an error — just return to idle.
      if (!msg.includes("cancelled")) {
        setError(
          e instanceof IPCError
            ? e.message
            : t("localFailed", { error: String(e) }),
        );
      }
    } finally {
      unlisten?.();
      setPhase({ stage: "idle" });
    }
  }

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-1 flex items-center gap-2">
        <Cpu size={18} className="text-[var(--color-accent-400)]" />
        <h2 className="text-[var(--text-ui-lg)] font-semibold">
          {t("localTitle")}
        </h2>
        <span className="flex items-center gap-1 rounded-full bg-[var(--color-success)]/15 px-2 py-0.5 text-[10px] text-[var(--color-success)]">
          <ShieldCheck size={10} /> {t("localPrivacyBadge")}
        </span>
      </div>
      <p className="mb-4 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        {t("localIntro")}
      </p>

      <div className="rounded-lg border border-[var(--color-accent-600)]/40 bg-[var(--color-accent-500)]/5 p-3">
        {modelReady ? (
          <>
            <button
              type="button"
              onClick={doTranscribe}
              disabled={busy}
              className="flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-4 py-2 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-50"
            >
              {busy ? (
                <Loader2 size={15} className="animate-spin" />
              ) : (
                <Cpu size={15} />
              )}
              {phase.stage === "extracting"
                ? t("localExtracting")
                : phase.stage === "preparing"
                  ? t("localLoadingModel")
                  : phase.stage === "running"
                    ? t("localTranscribingPct", {
                        pct: Math.round(phase.fraction * 100),
                      })
                    : t("localTranscribe")}
            </button>

            {phase.stage === "running" && (
              <div className="mt-3 flex items-center gap-2">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[var(--color-bg-surface)]">
                  <div
                    className="h-full bg-[var(--color-accent-500)] transition-[width]"
                    style={{ width: `${Math.round(phase.fraction * 100)}%` }}
                  />
                </div>
                <button
                  type="button"
                  onClick={() => void ipc.asr.cancelTranscribe()}
                  title={t("actionCancel")}
                  aria-label={t("actionCancel")}
                  className="text-[var(--color-fg-muted)] opacity-70 hover:opacity-100"
                >
                  <X size={12} />
                </button>
              </div>
            )}

            <p className="mt-2 text-[10px] text-[var(--color-fg-subtle)]">
              {t("localUsingModelHint")}
            </p>
            {error && (
              <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-danger)]">
                {error}
              </p>
            )}
          </>
        ) : (
          <p className="flex items-center gap-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            <Download size={14} />
            {t("localNeedModel")}
          </p>
        )}
      </div>
    </div>
  );
}
