import { useState } from "react";
import { Download, Settings as SettingsIcon, Captions, FileVideo, Clock, Cpu } from "lucide-react";

import { CaptionEditor } from "@/features/editor/CaptionEditor";
import { ImportScreen } from "@/features/project/ImportScreen";
import { ModelPicker } from "@/features/transcribe/ModelPicker";
import { Waveform } from "@/components/Waveform";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { ipc } from "@/lib/ipc";
import type { Project, WaveformData, WhisperModel } from "@/lib/bindings";
import { cn } from "@/lib/cn";

type Tab = "transcribe" | "editor" | "export";

function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [tab, setTab] = useState<Tab>("editor");
  const [exported, setExported] = useState<{ format: string; content: string } | null>(null);
  const [model, setModel] = useState<WhisperModel | null>("large-v3-turbo");

  // Import screen until a project exists. "Try the demo" loads SAMPLE_PROJECT
  // so the editor is explorable without a real video / ffmpeg.
  if (!project) {
    return (
      <div className="flex h-screen w-screen flex-col bg-[var(--color-bg)] text-[var(--color-fg)]">
        <div className="flex-1">
          <ImportScreen onProjectReady={setProject} />
        </div>
        <div className="border-t border-[var(--color-border)] px-6 py-3 text-center">
          <button
            type="button"
            onClick={() => setProject(SAMPLE_PROJECT)}
            className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] underline-offset-4 hover:text-[var(--color-accent-400)] hover:underline"
          >
            …eller utforsk demo-prosjektet (uten video)
          </button>
        </div>
      </div>
    );
  }

  async function doExport(format: "srt" | "vtt" | "ass" | "txt") {
    if (!project) return;
    const content =
      format === "srt" ? await ipc.exporters.srt(project, true)
      : format === "vtt" ? await ipc.exporters.vtt(project, true)
      : format === "ass" ? await ipc.exporters.ass(project)
      : await ipc.exporters.txt(project, true);
    setExported({ format, content });
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--color-bg)] text-[var(--color-fg)]">
      {/* Sidebar */}
      <nav className="flex w-14 flex-col items-center gap-1 border-r border-[var(--color-border)] bg-[var(--color-bg-elevated)] py-3">
        <button
          type="button"
          onClick={() => setProject(null)}
          title="Tilbake til import"
          className="mb-3 grid h-9 w-9 place-items-center rounded-lg bg-[var(--color-accent-600)] font-bold text-[var(--color-neutral-950)]"
        >
          V
        </button>
        <NavIcon active={tab === "transcribe"} onClick={() => setTab("transcribe")} title="Transkriber">
          <Cpu size={18} />
        </NavIcon>
        <NavIcon active={tab === "editor"} onClick={() => setTab("editor")} title="Editor">
          <Captions size={18} />
        </NavIcon>
        <NavIcon active={tab === "export"} onClick={() => setTab("export")} title="Eksport">
          <Download size={18} />
        </NavIcon>
        <div className="flex-1" />
        <NavIcon active={false} onClick={() => {}} title="Innstillinger">
          <SettingsIcon size={18} />
        </NavIcon>
      </nav>

      {/* Main */}
      <main className="flex flex-1 flex-col overflow-hidden">
        <ProjectHeader project={project} />
        <div className="flex-1 overflow-y-auto">
          {tab === "transcribe" ? (
            <div className="p-6">
              <ModelPicker selected={model} onSelect={setModel} />
            </div>
          ) : tab === "editor" ? (
            <CaptionEditor project={project} />
          ) : (
            <ExportPanel exported={exported} onExport={doExport} />
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
    levels: [Array.from({ length: 240 }, (_, i) => {
      const env = Math.sin((i / 240) * Math.PI * 6) * 0.6 + Math.sin(i * 0.7) * 0.3;
      return { min: -Math.abs(env), max: Math.abs(env) };
    })],
  };
  return (
    <div className="border-b border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-5 py-2.5">
      <div className="mb-2 flex items-center gap-2 text-[var(--text-ui-sm)]">
        <FileVideo size={14} className="text-[var(--color-fg-muted)]" />
        <span className="font-medium">{project.name}</span>
        {project.video_width > 0 && (
          <span className="text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
            {project.video_width}×{project.video_height} · {project.video_fps.toFixed(2)} fps
          </span>
        )}
        <span className="flex items-center gap-1 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
          <Clock size={11} /> {fmtDuration(project.video_duration_ms)}
        </span>
      </div>
      <Waveform data={demoWaveform} durationMs={project.video_duration_ms} height={64} />
    </div>
  );
}

function NavIcon({
  active, onClick, title, children,
}: { active: boolean; onClick: () => void; title: string; children: React.ReactNode }) {
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

function ExportPanel({
  exported, onExport,
}: {
  exported: { format: string; content: string } | null;
  onExport: (format: "srt" | "vtt" | "ass" | "txt") => void;
}) {
  const formats: Array<{ id: "srt" | "vtt" | "ass" | "txt"; label: string; desc: string }> = [
    { id: "srt", label: "SRT", desc: "Universal — YouTube, de fleste spillere" },
    { id: "vtt", label: "VTT", desc: "Web-standard, med talere" },
    { id: "ass", label: "ASS", desc: "Full styling — Aegisub, burn-in" },
    { id: "txt", label: "TXT", desc: "Ren transkripsjon, ingen tidskoder" },
  ];
  return (
    <div className="flex h-full overflow-hidden">
      <div className="w-72 shrink-0 space-y-2 border-r border-[var(--color-border)] p-4">
        {formats.map((f) => (
          <button
            key={f.id}
            type="button"
            onClick={() => onExport(f.id)}
            className="flex w-full flex-col rounded-md border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-2.5 text-left transition-colors hover:border-[var(--color-accent-600)]"
          >
            <span className="font-mono text-[var(--text-ui-sm)] font-semibold text-[var(--color-accent-400)]">
              {f.label}
            </span>
            <span className="text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">{f.desc}</span>
          </button>
        ))}
      </div>
      <div className="flex-1 overflow-auto p-4">
        {exported ? (
          <pre className="whitespace-pre-wrap rounded-md bg-[var(--color-bg-elevated)] p-4 font-mono text-[var(--text-ui-xs)] leading-relaxed">
            {exported.content}
          </pre>
        ) : (
          <div className="grid h-full place-items-center text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
            Velg et format til venstre for å se eksporten.
          </div>
        )}
      </div>
    </div>
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
