import { useCallback, useEffect, useState } from "react";
import { appDataDir, join } from "@tauri-apps/api/path";
import {
  Download,
  Settings as SettingsIcon,
  BookText,
  Captions,
  FileVideo,
  Clock,
  Cpu,
  Palette,
  Wand2,
  Sparkles,
  Lightbulb,
  Languages,
  Users,
  RefreshCw,
  GalleryHorizontalEnd,
  Scissors,
} from "lucide-react";

import { CaptionEditor } from "@/features/editor/CaptionEditor";
import { Timeline } from "@/features/timeline/Timeline";
import { ContextPanel } from "@/features/context/ContextPanel";
import { SettingsPanel } from "@/features/settings/SettingsPanel";
import { Onboarding } from "@/features/onboarding/Onboarding";
import { ImportScreen } from "@/features/project/ImportScreen";
import { ModelPicker } from "@/features/transcribe/ModelPicker";
import { LocalPanel } from "@/features/transcribe/LocalPanel";
import { CloudPanel } from "@/features/transcribe/CloudPanel";
import { StyleEditor } from "@/features/style/StyleEditor";
import { ExportPanel } from "@/features/export/ExportPanel";
import { CleanupPanel } from "@/features/cleanup/CleanupPanel";
import { PolishPanel } from "@/features/polish/PolishPanel";
import { SuggestPanel } from "@/features/suggest/SuggestPanel";
import { ClipsPanel } from "@/features/clips/ClipsPanel";
import { TranslatePanel } from "@/features/translate/TranslatePanel";
import { SpeakersPanel } from "@/features/speakers/SpeakersPanel";
import { Waveform } from "@/components/Waveform";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { ipc } from "@/lib/ipc";
import type {
  DownloadProgress,
  Project,
  Style,
  WaveformData,
  WhisperModel,
} from "@/lib/bindings";
import { checkForUpdate, installAndRelaunch, type Update } from "@/lib/updater";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";

type Tab =
  | "transcribe"
  | "editor"
  | "timeline"
  | "context"
  | "speakers"
  | "polish"
  | "suggest"
  | "clips"
  | "translate"
  | "cleanup"
  | "style"
  | "export"
  | "settings";

function App() {
  const t = useT();
  const [project, setProject] = useState<Project | null>(null);
  const [tab, setTab] = useState<Tab>("editor");
  const [model, setModel] = useState<WhisperModel | null>("large-v3-turbo");
  const [update, setUpdate] = useState<Update | null>(null);
  const [downloadedModels, setDownloadedModels] = useState<WhisperModel[]>([]);
  const [downloading, setDownloading] = useState<{
    model: WhisperModel;
    progress: DownloadProgress;
  } | null>(null);
  const [onboarded, setOnboarded] = useState<boolean>(() => {
    try {
      return localStorage.getItem("sundayedit.onboarded") === "1";
    } catch {
      return false;
    }
  });

  // Check for a newer signed build once on launch (no-op outside Tauri /
  // before any release exists).
  useEffect(() => {
    checkForUpdate().then(setUpdate);
  }, []);

  const refreshDownloaded = useCallback(async () => {
    try {
      const dir = await join(await appDataDir(), "models");
      setDownloadedModels(await ipc.asr.downloadedModels(dir));
    } catch {
      // Not in Tauri (browser dev) — no local models dir; leave empty.
    }
  }, []);

  useEffect(() => {
    void refreshDownloaded();
  }, [refreshDownloaded]);

  // Fetch the chosen Whisper model on first use. Streams progress; refreshes
  // the downloaded set on completion.
  async function handleDownloadModel(m: WhisperModel) {
    let unlisten: (() => void) | undefined;
    try {
      const dir = await join(await appDataDir(), "models");
      setDownloading({
        model: m,
        progress: { downloaded_bytes: 0, total_bytes: null, fraction: null },
      });
      unlisten = await ipc.asr.onDownloadProgress((progress) =>
        setDownloading((d) =>
          d && d.model === m ? { model: m, progress } : d,
        ),
      );
      await ipc.asr.downloadModel(dir, m);
      await refreshDownloaded();
    } catch (e) {
      console.error("model download failed", e);
    } finally {
      unlisten?.();
      setDownloading(null);
    }
  }

  // First run (no project yet): walk onboarding once, then the import screen.
  if (!project && !onboarded) {
    return (
      <Onboarding
        selected={model}
        onSelect={setModel}
        downloadedModels={downloadedModels}
        downloading={downloading}
        onDownload={handleDownloadModel}
        onDone={() => {
          try {
            localStorage.setItem("sundayedit.onboarded", "1");
          } catch {
            /* private mode — onboarding just shows again next launch */
          }
          setOnboarded(true);
        }}
        onTryDemo={() => setProject(SAMPLE_PROJECT)}
      />
    );
  }

  // Import screen until a project exists. "Try the demo" loads SAMPLE_PROJECT
  // so the editor is explorable without a real video / ffmpeg.
  if (!project) {
    return (
      <div className="flex h-screen w-screen flex-col bg-[var(--color-bg)] text-[var(--color-fg)]">
        {update && (
          <UpdateBanner update={update} onDismiss={() => setUpdate(null)} />
        )}
        <div className="flex-1">
          <ImportScreen onProjectReady={setProject} />
        </div>
        <div className="border-t border-[var(--color-border)] px-6 py-3 text-center">
          <button
            type="button"
            onClick={() => setProject(SAMPLE_PROJECT)}
            className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] underline-offset-4 hover:text-[var(--color-accent-400)] hover:underline"
          >
            {t("importDemoLink")}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--color-bg)] text-[var(--color-fg)]">
      {update && (
        <UpdateBanner update={update} onDismiss={() => setUpdate(null)} />
      )}
      {/* Sidebar */}
      <nav className="flex w-14 flex-col items-center gap-1 border-r border-[var(--color-border)] bg-[var(--color-bg-elevated)] py-3">
        <button
          type="button"
          onClick={() => setProject(null)}
          title={t("navBackToImport")}
          className="mb-3 grid h-9 w-9 place-items-center rounded-lg bg-[var(--color-accent-600)] font-bold text-[var(--color-neutral-950)]"
        >
          V
        </button>
        <NavIcon
          active={tab === "transcribe"}
          onClick={() => setTab("transcribe")}
          title={t("navTranscribe")}
        >
          <Cpu size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "editor"}
          onClick={() => setTab("editor")}
          title={t("navEditor")}
        >
          <Captions size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "timeline"}
          onClick={() => setTab("timeline")}
          title={t("navTimeline")}
        >
          <GalleryHorizontalEnd size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "context"}
          onClick={() => setTab("context")}
          title={t("navContext")}
        >
          <BookText size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "speakers"}
          onClick={() => setTab("speakers")}
          title={t("navSpeakers")}
        >
          <Users size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "polish"}
          onClick={() => setTab("polish")}
          title={t("navPolish")}
        >
          <Sparkles size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "suggest"}
          onClick={() => setTab("suggest")}
          title={t("navSuggest")}
        >
          <Lightbulb size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "clips"}
          onClick={() => setTab("clips")}
          title={t("navClips")}
        >
          <Scissors size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "translate"}
          onClick={() => setTab("translate")}
          title={t("navTranslate")}
        >
          <Languages size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "cleanup"}
          onClick={() => setTab("cleanup")}
          title={t("navCleanup")}
        >
          <Wand2 size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "style"}
          onClick={() => setTab("style")}
          title={t("navStyle")}
        >
          <Palette size={18} />
        </NavIcon>
        <NavIcon
          active={tab === "export"}
          onClick={() => setTab("export")}
          title={t("navExport")}
        >
          <Download size={18} />
        </NavIcon>
        <div className="flex-1" />
        <NavIcon
          active={tab === "settings"}
          onClick={() => setTab("settings")}
          title={t("navSettings")}
        >
          <SettingsIcon size={18} />
        </NavIcon>
      </nav>

      {/* Main */}
      <main className="flex flex-1 flex-col overflow-hidden">
        <ProjectHeader project={project} />
        <div className="flex-1 overflow-y-auto">
          {tab === "transcribe" ? (
            <div className="space-y-10 p-6">
              <ModelPicker
                selected={model}
                onSelect={setModel}
                downloadedModels={downloadedModels}
                downloading={downloading}
                onDownload={handleDownloadModel}
              />
              <LocalPanel
                project={project}
                model={model}
                downloadedModels={downloadedModels}
                onProjectChange={setProject}
                onTranscribed={(captions) => {
                  // Functional merge so the audio_wav_path LocalPanel just set
                  // (via onProjectChange) isn't clobbered by a stale closure.
                  setProject((prev) => (prev ? { ...prev, captions } : prev));
                  setTab("editor");
                }}
              />
              <CloudPanel
                project={project}
                onTranscribed={(captions) => {
                  setProject((prev) => (prev ? { ...prev, captions } : prev));
                  setTab("editor");
                }}
              />
            </div>
          ) : tab === "editor" ? (
            <CaptionEditor
              key={project.id}
              project={project}
              onProjectChange={setProject}
            />
          ) : tab === "timeline" ? (
            <Timeline project={project} onProjectChange={setProject} />
          ) : tab === "context" ? (
            <ContextPanel project={project} onProjectChange={setProject} />
          ) : tab === "speakers" ? (
            <SpeakersPanel project={project} onProjectChange={setProject} />
          ) : tab === "polish" ? (
            <PolishPanel project={project} onProjectChange={setProject} />
          ) : tab === "suggest" ? (
            <SuggestPanel project={project} onProjectChange={setProject} />
          ) : tab === "clips" ? (
            <ClipsPanel project={project} onProjectChange={setProject} />
          ) : tab === "translate" ? (
            <TranslatePanel project={project} onProjectChange={setProject} />
          ) : tab === "cleanup" ? (
            <CleanupPanel project={project} onProjectChange={setProject} />
          ) : tab === "style" ? (
            <StyleEditor
              style={project.default_style}
              onChange={(s: Style) =>
                setProject({ ...project, default_style: s })
              }
            />
          ) : tab === "export" ? (
            <ExportPanel project={project} />
          ) : (
            <SettingsPanel />
          )}
        </div>
      </main>
    </div>
  );
}

function ProjectHeader({ project }: { project: Project }) {
  // Demo waveform data so the component is visible without real extraction.
  const demoWaveform: WaveformData = {
    sample_rate: 16000,
    total_samples: 16000 * 18,
    levels: [
      Array.from({ length: 240 }, (_, i) => {
        const env =
          Math.sin((i / 240) * Math.PI * 6) * 0.6 + Math.sin(i * 0.7) * 0.3;
        return { min: -Math.abs(env), max: Math.abs(env) };
      }),
    ],
  };
  return (
    <div className="border-b border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-5 py-2.5">
      <div className="mb-2 flex items-center gap-2 text-[var(--text-ui-sm)]">
        <FileVideo size={14} className="text-[var(--color-fg-muted)]" />
        <span className="font-medium">{project.name}</span>
        {project.video_width > 0 && (
          <span className="text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
            {project.video_width}×{project.video_height} ·{" "}
            {project.video_fps.toFixed(2)} fps
          </span>
        )}
        <span className="flex items-center gap-1 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
          <Clock size={11} /> {fmtDuration(project.video_duration_ms)}
        </span>
      </div>
      <Waveform
        data={demoWaveform}
        durationMs={project.video_duration_ms}
        height={64}
      />
    </div>
  );
}

function UpdateBanner({
  update,
  onDismiss,
}: {
  update: Update;
  onDismiss: () => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function install() {
    setBusy(true);
    setError(null);
    try {
      await installAndRelaunch(update);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-x-0 top-0 z-50 flex items-center justify-center gap-3 bg-[var(--color-accent-600)] px-4 py-1.5 text-[var(--text-ui-sm)] text-[var(--color-neutral-950)] shadow-md">
      <RefreshCw size={14} className={cn(busy && "animate-spin")} />
      <span className="font-medium">
        {error
          ? t("updateFailed", { error })
          : busy
            ? t("updateInstalling")
            : t("updateAvailable", { version: update.version })}
      </span>
      {!busy && !error && (
        <>
          <button
            type="button"
            onClick={install}
            className="rounded bg-[var(--color-neutral-950)]/15 px-2.5 py-0.5 font-semibold hover:bg-[var(--color-neutral-950)]/25"
          >
            {t("updateNow")}
          </button>
          <button
            type="button"
            onClick={onDismiss}
            className="px-1 opacity-70 hover:opacity-100"
            aria-label={t("actionClose")}
          >
            {t("updateLater")}
          </button>
        </>
      )}
    </div>
  );
}

function NavIcon({
  active,
  onClick,
  title,
  children,
}: {
  active: boolean;
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      aria-label={title}
      className={cn(
        "grid h-10 w-10 place-items-center rounded-lg transition-colors",
        active
          ? "bg-[var(--color-bg-surface)] text-[var(--color-accent-400)]"
          : "text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)]",
      )}
    >
      {children}
    </button>
  );
}

function fmtDuration(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
    : `${m}:${String(s).padStart(2, "0")}`;
}

export default App;
