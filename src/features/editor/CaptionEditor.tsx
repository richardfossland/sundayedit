/**
 * Caption editor — the heart of Verbatim (Phase 3.2 + 3.3).
 *
 * Demonstrates the killer feature: confidence highlighting. Each word is
 * rendered with a tier-based background. Focus mode dims captions whose
 * words are all high-confidence so the eye snaps to what needs review.
 *
 * This is a working preview against SAMPLE_PROJECT. The video player,
 * timeline, and real ASR pipeline land in Phases 1–2; the operations are
 * already wired to the Rust backend via ipc.ops.
 */

import { useMemo, useState } from "react";
import { Eye, Sparkles, ChevronRight } from "lucide-react";

import { confidenceTier, type Caption, type Project, type Word } from "@/lib/bindings";
import { cn } from "@/lib/cn";

interface Props {
  project: Project;
}

export function CaptionEditor({ project }: Props) {
  const [focusMode, setFocusMode] = useState(false);
  const [threshold, setThreshold] = useState(70);

  const stats = useMemo(() => {
    let uncertain = 0;
    let total = 0;
    for (const c of project.captions) {
      for (const word of c.words) {
        total++;
        if (!word.locked && !word.edited && word.confidence < threshold) uncertain++;
      }
    }
    return { uncertain, total };
  }, [project, threshold]);

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <header className="flex items-center gap-4 border-b border-[var(--color-border)] px-5 py-3">
        <div>
          <h1 className="text-[var(--text-ui-lg)] font-semibold">{project.name}</h1>
          <p className="text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">
            {project.captions.length} captions · sprog: {project.language}
          </p>
        </div>
        <div className="flex-1" />

        {/* Threshold slider */}
        <label className="flex items-center gap-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
          <span>Usikkerhetsterskel</span>
          <input
            type="range"
            min={40}
            max={95}
            value={threshold}
            onChange={(e) => setThreshold(Number(e.target.value))}
            className="accent-[var(--color-accent-500)]"
          />
          <span className="w-8 font-mono tabular-nums">{threshold}</span>
        </label>

        <button
          type="button"
          onClick={() => setFocusMode((f) => !f)}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[var(--text-ui-sm)] font-medium transition-colors",
            focusMode
              ? "bg-[var(--color-accent-500)] text-[var(--color-neutral-950)]"
              : "bg-[var(--color-bg-surface)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
          )}
          title="Fokus-modus: demp sikre captions, fremhev de som trenger gjennomgang"
        >
          <Eye size={14} />
          Fokus-modus
        </button>
      </header>

      {/* Review progress strip */}
      <div className="flex items-center gap-3 border-b border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-5 py-2">
        <Sparkles size={14} className="text-[var(--color-accent-400)]" />
        <span className="text-[var(--text-ui-sm)]">
          <strong className="font-semibold tabular-nums">{stats.uncertain}</strong>{" "}
          <span className="text-[var(--color-fg-muted)]">
            usikre ord av {stats.total} totalt (under {threshold}% sikkerhet)
          </span>
        </span>
        <div className="ml-2 h-1.5 w-48 overflow-hidden rounded-full bg-[var(--color-bg-surface)]">
          <div
            className="h-full bg-[var(--color-accent-500)]"
            style={{ width: `${100 - (stats.uncertain / Math.max(1, stats.total)) * 100}%` }}
          />
        </div>
        <ConfidenceLegend />
      </div>

      {/* Caption list */}
      <div className="flex-1 overflow-y-auto px-5 py-4">
        <ul className="mx-auto max-w-3xl space-y-1.5">
          {project.captions.map((caption) => (
            <CaptionRow
              key={caption.id}
              caption={caption}
              threshold={threshold}
              dimmed={focusMode && allConfident(caption, threshold)}
            />
          ))}
        </ul>
      </div>
    </div>
  );
}

function CaptionRow({
  caption,
  threshold,
  dimmed,
}: {
  caption: Caption;
  threshold: number;
  dimmed: boolean;
}) {
  return (
    <li
      className={cn(
        "flex gap-3 rounded-md border border-transparent px-3 py-2 transition-opacity hover:border-[var(--color-border)]",
        dimmed && "opacity-30 hover:opacity-100",
      )}
    >
      <span className="w-20 shrink-0 pt-0.5 font-mono text-[var(--text-ui-xs)] tabular-nums text-[var(--color-fg-subtle)]">
        {fmtTime(caption.start_ms)}
      </span>
      <p className="flex-1 text-[var(--text-ui-md)] leading-relaxed">
        {caption.words.map((word, i) => (
          <WordSpan key={i} word={word} threshold={threshold} />
        ))}
      </p>
    </li>
  );
}

function WordSpan({ word, threshold }: { word: Word; threshold: number }) {
  const tier = confidenceTier(word);
  // The threshold control lets the user widen/narrow what "uncertain"
  // means; below it we always show at least tier-2 emphasis.
  const effectiveTier =
    !word.locked && !word.edited && word.confidence < threshold
      ? Math.max(tier, 2)
      : tier;

  const title =
    word.alternates.length > 0
      ? `${word.confidence.toFixed(0)}% — alternativer: ${word.alternates.map((a) => a.text).join(", ")}`
      : `${word.confidence.toFixed(0)}% sikkerhet`;

  return (
    <>
      <span
        className={cn(
          "word",
          `word-tier-${effectiveTier}`,
          word.edited && "is-edited",
          word.locked && "is-locked",
        )}
        title={title}
      >
        {word.text}
      </span>{" "}
    </>
  );
}

function ConfidenceLegend() {
  const tiers = [
    { tier: 1, label: "Sikker" },
    { tier: 2, label: "Litt usikker" },
    { tier: 3, label: "Usikker" },
    { tier: 4, label: "Svært usikker" },
  ];
  return (
    <div className="ml-auto flex items-center gap-3 text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">
      {tiers.map((t) => (
        <span key={t.tier} className="flex items-center gap-1.5">
          <span className={cn("word inline-block h-3 w-4 rounded", `word-tier-${t.tier}`)} />
          {t.label}
        </span>
      ))}
    </div>
  );
}

function allConfident(caption: Caption, threshold: number): boolean {
  return caption.words.every(
    (w) => w.locked || w.edited || w.confidence >= threshold,
  );
}

function fmtTime(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  const cs = Math.floor((ms % 1000) / 10);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(cs).padStart(2, "0")}`;
}

export { ChevronRight };
