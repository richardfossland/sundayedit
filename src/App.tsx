import { useState } from "react";
import { FileVideo, Download, Settings as SettingsIcon, Captions } from "lucide-react";

import { CaptionEditor } from "@/features/editor/CaptionEditor";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { ipc } from "@/lib/ipc";
import { cn } from "@/lib/cn";

type Tab = "editor" | "export";

function App() {
  const [tab, setTab] = useState<Tab>("editor");
  const [exported, setExported] = useState<{ format: string; content: string } | null>(null);

  async function doExport(format: "srt" | "vtt" | "ass" | "txt") {
    const content =
      format === "srt" ? await ipc.exporters.srt(SAMPLE_PROJECT, true)
      : format === "vtt" ? await ipc.exporters.vtt(SAMPLE_PROJECT, true)
      : format === "ass" ? await ipc.exporters.ass(SAMPLE_PROJECT)
      : await ipc.exporters.txt(SAMPLE_PROJECT, true);
    setExported({ format, content });
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--color-bg)] text-[var(--color-fg)]">
      {/* Sidebar */}
      <nav className="flex w-14 flex-col items-center gap-1 border-r border-[var(--color-border)] bg-[var(--color-bg-elevated)] py-3">
        <div className="mb-3 grid h-9 w-9 place-items-center rounded-lg bg-[var(--color-accent-600)] font-bold text-[var(--color-neutral-950)]">
          V
        </div>
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
      <main className="flex-1 overflow-hidden">
        {tab === "editor" ? (
          <CaptionEditor project={SAMPLE_PROJECT} />
        ) : (
          <ExportPanel exported={exported} onExport={doExport} />
        )}
      </main>
    </div>
  );
}

function NavIcon({
  active, onClick, title, children,
}: {
  active: boolean; onClick: () => void; title: string; children: React.ReactNode;
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

function ExportPanel({
  exported,
  onExport,
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
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-2 border-b border-[var(--color-border)] px-5 py-3">
        <FileVideo size={16} className="text-[var(--color-fg-muted)]" />
        <h1 className="text-[var(--text-ui-lg)] font-semibold">Eksport</h1>
      </header>
      <div className="flex flex-1 overflow-hidden">
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
            <pre className="whitespace-pre-wrap rounded-md bg-[var(--color-bg-elevated)] p-4 font-mono text-[var(--text-ui-xs)] leading-relaxed text-[var(--color-fg)]">
              {exported.content}
            </pre>
          ) : (
            <div className="grid h-full place-items-center text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              Velg et format til venstre for å se eksporten av demo-prosjektet.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default App;
