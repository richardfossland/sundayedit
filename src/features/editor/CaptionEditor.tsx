/**
 * Caption editor — the heart of SundayEdit (Phase 3.2 + 3.3 + 3.4).
 *
 * Interactive now, not just a viewer:
 *   - Click a word → inline edit (Enter commits via Rust op_edit_word)
 *   - Click an uncertain word's chevron → popover with ASR alternates +
 *     "Mark as correct" (lock). Picking an alternate calls
 *     op_accept_alternate.
 *   - Tab / Shift-Tab → jump to next / previous uncertain word (linear
 *     review, Phase 3.3)
 *   - Focus mode dims all-confident captions
 *   - "Fix terms" runs the glossary auto-correction pass (Phase 3.4)
 *   - ⌘Z / ⌘⇧Z undo/redo (via the shared useProjectStore)
 *
 * All edits go through the Rust pure-function operations and flow through
 * the shared project store, so undo/redo and invariant-validation come for
 * free — and timeline drags share the very same undo stack.
 */

import { useEffect, useMemo, useState } from "react";
import {
  Eye,
  Sparkles,
  Undo2,
  Redo2,
  Lock,
  Check,
  ChevronRight,
  WandSparkles,
  Merge,
  Trash2,
  User,
  MoveHorizontal,
  X,
} from "lucide-react";

import {
  confidenceTier,
  type Caption,
  type Project,
  type Speaker,
  type Word,
} from "@/lib/bindings";
import { ipc } from "@/lib/ipc";
import {
  useProjectStore,
  selectCanUndo,
  selectCanRedo,
} from "@/lib/useProjectStore";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";
import {
  EMPTY_SELECTION,
  clear as clearSelection,
  count as selectionCount,
  isContiguous,
  isSelected,
  orderedSelection,
  selectAll,
  selectRange,
  toggle as toggleSelection,
  type Selection,
} from "./selection";

interface WordRef {
  captionId: string;
  wordIndex: number;
}
function sameRef(a: WordRef, b: WordRef) {
  return a.captionId === b.captionId && a.wordIndex === b.wordIndex;
}

/**
 * Reads the open project from the shared store. App only mounts us once a
 * project exists, but the store is nullable — guard so the inner editor always
 * has a non-null Project.
 */
export function CaptionEditor() {
  const project = useProjectStore((s) => s.project);
  if (!project) return null;
  return <CaptionEditorInner project={project} />;
}

function CaptionEditorInner({ project }: { project: Project }) {
  const t = useT();
  // All undoable edits flow through the shared store's history.
  const run = useProjectStore((s) => s.run);
  const undo = useProjectStore((s) => s.undo);
  const redo = useProjectStore((s) => s.redo);
  const busy = useProjectStore((s) => s.busy);
  const canUndo = useProjectStore(selectCanUndo);
  const canRedo = useProjectStore(selectCanRedo);

  const [focusMode, setFocusMode] = useState(false);
  const [threshold, setThreshold] = useState(70);
  const [cursor, setCursor] = useState<WordRef | null>(null);
  const [editing, setEditing] = useState<WordRef | null>(null);
  const [glossaryToast, setGlossaryToast] = useState<string | null>(null);

  // ── Multi-select (bulk operations) ──────────────────────────────────────────
  const [selection, setSelection] = useState<Selection>(EMPTY_SELECTION);
  const [anchorId, setAnchorId] = useState<string | null>(null);

  const orderedIds = useMemo(
    () => project.captions.map((c) => c.id),
    [project.captions],
  );

  // Drop ids that no longer exist after merge/delete so the bar can't act on
  // stale captions.
  useEffect(() => {
    const live = new Set(orderedIds);
    setSelection((prev) => {
      let changed = false;
      const next = new Set<string>();
      for (const id of prev) {
        if (live.has(id)) next.add(id);
        else changed = true;
      }
      return changed ? next : prev;
    });
  }, [orderedIds]);

  function onSelectCaption(id: string, range: boolean) {
    if (range) {
      setSelection((prev) => selectRange(prev, orderedIds, anchorId, id));
    } else {
      setSelection((prev) => toggleSelection(prev, id));
      setAnchorId(id);
    }
  }

  const stats = useMemo(() => {
    let uncertain = 0,
      total = 0;
    for (const c of project.captions) {
      for (const word of c.words) {
        total++;
        if (!word.locked && !word.edited && word.confidence < threshold)
          uncertain++;
      }
    }
    return { uncertain, total };
  }, [project, threshold]);

  const uncertainRefs = useMemo(() => {
    const refs: WordRef[] = [];
    project.captions.forEach((c) => {
      c.words.forEach((w, wi) => {
        if (!w.locked && !w.edited && w.confidence < threshold) {
          refs.push({ captionId: c.id, wordIndex: wi });
        }
      });
    });
    return refs;
  }, [project, threshold]);

  // Tab / Shift-Tab → next / previous uncertain word.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Tab") return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.isContentEditable)) return;
      if (uncertainRefs.length === 0) return;
      e.preventDefault();
      const curIdx = cursor
        ? uncertainRefs.findIndex((r) => sameRef(r, cursor))
        : -1;
      const nextIdx = e.shiftKey
        ? curIdx <= 0
          ? uncertainRefs.length - 1
          : curIdx - 1
        : (curIdx + 1) % uncertainRefs.length;
      setCursor(uncertainRefs[nextIdx]);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [uncertainRefs, cursor]);

  async function editWord(ref: WordRef, newText: string) {
    setEditing(null);
    if (newText.trim().length === 0) return;
    await run((p) =>
      ipc.ops.editWord(p, ref.captionId, ref.wordIndex, newText),
    );
  }
  async function lockWord(ref: WordRef) {
    await run((p) => ipc.ops.lockWord(p, ref.captionId, ref.wordIndex, true));
  }
  async function acceptAlternate(ref: WordRef, altIndex: number) {
    await run((p) =>
      ipc.ops.acceptAlternate(p, ref.captionId, ref.wordIndex, altIndex),
    );
  }
  async function applyGlossary() {
    await run(async (p) => {
      const result = await ipc.ops.applyGlossary(p);
      setGlossaryToast(
        result.corrections.length === 0
          ? t("editorGlossaryNoHits")
          : t("editorGlossaryCorrected", { n: result.corrections.length }),
      );
      return result.project;
    });
  }

  // ── Bulk actions (each flows through history → undo + export see it) ─────────
  const selectedIds = useMemo(
    () => orderedSelection(selection, orderedIds),
    [selection, orderedIds],
  );
  const contiguous = useMemo(
    () => isContiguous(selection, orderedIds),
    [selection, orderedIds],
  );

  async function bulkMerge() {
    if (!contiguous) return;
    await run((p) => ipc.ops.mergeCaptions(p, selectedIds));
  }
  async function bulkDelete() {
    if (selectedIds.length === 0) return;
    await run((p) => ipc.cleanup.bulkDelete(p, selectedIds));
    setSelection(clearSelection());
  }
  async function bulkSetSpeaker(speakerId: string | null) {
    if (selectedIds.length === 0) return;
    await run((p) => ipc.cleanup.bulkSetSpeaker(p, selectedIds, speakerId));
  }
  async function shiftWholeProject() {
    const raw = window.prompt(t("editorShiftAllPrompt"), "0");
    if (raw === null) return;
    const offset = Number(raw);
    if (!Number.isFinite(offset) || offset === 0) return;
    await run((p) => ipc.ops.shiftAll(p, offset));
  }

  useEffect(() => {
    if (!glossaryToast) return;
    const t = setTimeout(() => setGlossaryToast(null), 4000);
    return () => clearTimeout(t);
  }, [glossaryToast]);

  return (
    <div className="relative flex h-full flex-col">
      {/* Toolbar */}
      <header className="flex items-center gap-3 border-b border-[var(--color-border)] px-5 py-3">
        <h1 className="text-[var(--text-ui-lg)] font-semibold">
          {t("navEditor")}
        </h1>
        <div className="flex items-center gap-0.5">
          <IconBtn
            title={t("editorUndo")}
            disabled={!canUndo || busy}
            onClick={undo}
          >
            <Undo2 size={15} />
          </IconBtn>
          <IconBtn
            title={t("editorRedo")}
            disabled={!canRedo || busy}
            onClick={redo}
          >
            <Redo2 size={15} />
          </IconBtn>
        </div>
        <div className="flex-1" />

        {project.glossary.length > 0 && (
          <button
            type="button"
            onClick={applyGlossary}
            disabled={busy}
            className="flex items-center gap-1.5 rounded-md bg-[var(--color-bg-surface)] px-3 py-1.5 text-[var(--text-ui-sm)] font-medium text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] disabled:opacity-50"
            title={t("editorFixTermsTitle")}
          >
            <WandSparkles size={14} /> {t("editorFixTerms")}
          </button>
        )}

        <label className="flex items-center gap-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
          <span>{t("editorThreshold")}</span>
          <input
            type="range"
            min={40}
            max={95}
            value={threshold}
            onChange={(e) => setThreshold(Number(e.target.value))}
            className="accent-[var(--color-accent-500)]"
          />
          <span className="w-7 font-mono tabular-nums">{threshold}</span>
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
        >
          <Eye size={14} /> {t("editorFocus")}
        </button>
      </header>

      {/* Review progress */}
      <div className="flex items-center gap-3 border-b border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-5 py-2">
        <Sparkles size={14} className="text-[var(--color-accent-400)]" />
        <span className="text-[var(--text-ui-sm)]">
          <strong className="font-semibold tabular-nums">
            {stats.uncertain}
          </strong>{" "}
          <span className="text-[var(--color-fg-muted)]">
            {t("editorUncertainOf", { total: stats.total })}
          </span>
        </span>
        <div className="ml-2 h-1.5 w-40 overflow-hidden rounded-full bg-[var(--color-bg-surface)]">
          <div
            className="h-full bg-[var(--color-accent-500)] transition-all"
            style={{
              width: `${100 - (stats.uncertain / Math.max(1, stats.total)) * 100}%`,
            }}
          />
        </div>
        <span className="text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
          <kbd className="rounded border border-[var(--color-border)] px-1 font-mono">
            Tab
          </kbd>{" "}
          {t("editorTabToNext")}
        </span>
        <ConfidenceLegend />
      </div>

      {/* Caption list */}
      <div className="flex-1 overflow-y-auto px-5 py-4">
        <ul className="mx-auto max-w-3xl space-y-1.5">
          {project.captions.map((caption) => (
            <CaptionRow
              key={caption.id}
              caption={caption}
              speaker={
                project.speakers.length >= 2 && caption.speaker_id
                  ? (project.speakers.find(
                      (s) => s.id === caption.speaker_id,
                    ) ?? null)
                  : null
              }
              threshold={threshold}
              dimmed={focusMode && allConfident(caption, threshold)}
              selected={isSelected(selection, caption.id)}
              cursor={cursor}
              editing={editing}
              busy={busy}
              onSelect={onSelectCaption}
              onCursor={setCursor}
              onStartEdit={setEditing}
              onCommitEdit={editWord}
              onLock={lockWord}
              onAcceptAlternate={acceptAlternate}
            />
          ))}
        </ul>
      </div>

      {selectionCount(selection) > 0 && (
        <BulkActionBar
          count={selectionCount(selection)}
          contiguous={contiguous}
          speakers={project.speakers}
          busy={busy}
          onSelectAll={() => setSelection(selectAll(orderedIds))}
          onClear={() => {
            setSelection(clearSelection());
            setAnchorId(null);
          }}
          onMerge={bulkMerge}
          onDelete={bulkDelete}
          onSetSpeaker={bulkSetSpeaker}
          onShiftAll={shiftWholeProject}
        />
      )}

      {glossaryToast && (
        <div className="pointer-events-none absolute bottom-6 left-1/2 -translate-x-1/2 rounded-lg bg-[var(--color-bg-surface)] px-4 py-2 text-[var(--text-ui-sm)] shadow-[var(--shadow-popover)]">
          {glossaryToast}
        </div>
      )}
    </div>
  );
}

function CaptionRow({
  caption,
  speaker,
  threshold,
  dimmed,
  selected,
  cursor,
  editing,
  busy,
  onSelect,
  onCursor,
  onStartEdit,
  onCommitEdit,
  onLock,
  onAcceptAlternate,
}: {
  caption: Caption;
  speaker: Speaker | null;
  threshold: number;
  dimmed: boolean;
  selected: boolean;
  cursor: WordRef | null;
  editing: WordRef | null;
  busy: boolean;
  onSelect: (id: string, range: boolean) => void;
  onCursor: (r: WordRef) => void;
  onStartEdit: (r: WordRef) => void;
  onCommitEdit: (r: WordRef, text: string) => void;
  onLock: (r: WordRef) => void;
  onAcceptAlternate: (r: WordRef, altIndex: number) => void;
}) {
  const t = useT();
  return (
    <li
      className={cn(
        "flex gap-3 rounded-md border px-3 py-2 transition-opacity",
        selected
          ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/5"
          : "border-transparent hover:border-[var(--color-border)]",
        dimmed && "opacity-30 hover:opacity-100",
      )}
    >
      <input
        type="checkbox"
        checked={selected}
        title={t("editorSelectToggle")}
        aria-label={t("editorSelectToggle")}
        onClick={(e) => {
          // Shift-click selects the range from the last anchor.
          e.preventDefault();
          onSelect(caption.id, e.shiftKey);
        }}
        onChange={() => {
          /* handled in onClick to read the shift modifier */
        }}
        className="mt-1 shrink-0 accent-[var(--color-accent-500)]"
      />
      <span className="w-20 shrink-0 pt-0.5 font-mono text-[var(--text-ui-xs)] tabular-nums text-[var(--color-fg-subtle)]">
        {fmtTime(caption.start_ms)}
      </span>
      <p className="flex-1 text-[var(--text-ui-md)] leading-relaxed">
        {speaker && (
          <span
            className="mr-1.5 inline-flex items-center gap-1 align-baseline text-[var(--text-ui-xs)] font-semibold"
            style={{ color: speaker.color_hex ?? "var(--color-accent-400)" }}
          >
            <span
              className="inline-block h-2 w-2 rounded-full"
              style={{
                backgroundColor: speaker.color_hex ?? "var(--color-accent-400)",
              }}
            />
            {speaker.display_name}:
          </span>
        )}
        {caption.words.map((word, i) => {
          const ref: WordRef = { captionId: caption.id, wordIndex: i };
          return (
            <WordSpan
              key={i}
              word={word}
              threshold={threshold}
              isCursor={cursor != null && sameRef(cursor, ref)}
              isEditing={editing != null && sameRef(editing, ref)}
              busy={busy}
              onFocus={() => onCursor(ref)}
              onStartEdit={() => onStartEdit(ref)}
              onCommitEdit={(text) => onCommitEdit(ref, text)}
              onLock={() => onLock(ref)}
              onAcceptAlternate={(ai) => onAcceptAlternate(ref, ai)}
            />
          );
        })}
      </p>
    </li>
  );
}

function WordSpan({
  word,
  threshold,
  isCursor,
  isEditing,
  busy,
  onFocus,
  onStartEdit,
  onCommitEdit,
  onLock,
  onAcceptAlternate,
}: {
  word: Word;
  threshold: number;
  isCursor: boolean;
  isEditing: boolean;
  busy: boolean;
  onFocus: () => void;
  onStartEdit: () => void;
  onCommitEdit: (text: string) => void;
  onLock: () => void;
  onAcceptAlternate: (altIndex: number) => void;
}) {
  const t = useT();
  const [popoverOpen, setPopoverOpen] = useState(false);
  const tier = confidenceTier(word);
  const effectiveTier =
    !word.locked && !word.edited && word.confidence < threshold
      ? Math.max(tier, 2)
      : tier;
  const isUncertain = effectiveTier >= 3;

  if (isEditing) {
    return (
      <input
        autoFocus
        defaultValue={word.text}
        onBlur={(e) => onCommitEdit(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onCommitEdit(e.currentTarget.value);
          }
          if (e.key === "Escape") {
            e.preventDefault();
            onCommitEdit(word.text);
          }
        }}
        className="mx-0.5 inline rounded border border-[var(--color-accent-500)] bg-[var(--color-bg-input)] px-1 text-[var(--text-ui-md)] outline-none"
        style={{ width: `${Math.max(2, word.text.length + 1)}ch` }}
      />
    );
  }

  return (
    <span className="relative inline">
      <span
        role="button"
        tabIndex={0}
        onClick={() => {
          onFocus();
          onStartEdit();
        }}
        onFocus={onFocus}
        className={cn(
          "word cursor-text",
          `word-tier-${effectiveTier}`,
          word.edited && "is-edited",
          word.locked && "is-locked",
          isCursor &&
            "ring-2 ring-[var(--color-accent-500)] ring-offset-1 ring-offset-[var(--color-bg)]",
        )}
        title={t("editorConfidencePct", { pct: word.confidence.toFixed(0) })}
      >
        {word.text}
      </span>
      {word.polished && (
        <span
          className="ml-0.5 inline-block h-1 w-1 -translate-y-[0.5em] rounded-full bg-[var(--color-accent-400)] align-top"
          title={t("editorPolishedDot")}
          aria-label={t("editorPolishedDot")}
        />
      )}
      {isUncertain && !busy && (
        <button
          type="button"
          onClick={() => setPopoverOpen((o) => !o)}
          className="ml-0.5 inline-flex translate-y-0.5 text-[var(--color-fg-subtle)] hover:text-[var(--color-accent-400)]"
          title={t("editorShowAlternates")}
          aria-label={t("editorShowAlternates")}
        >
          <ChevronRight
            size={12}
            className={cn("transition-transform", popoverOpen && "rotate-90")}
          />
        </button>
      )}
      {popoverOpen && (
        <span className="absolute left-0 top-full z-20 mt-1 block w-56 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-1.5 shadow-[var(--shadow-popover)]">
          <span className="mb-1 block px-2 py-1 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
            {t("editorConfidencePct", { pct: word.confidence.toFixed(0) })}
          </span>
          {word.alternates.length > 0 ? (
            word.alternates.map((alt, ai) => (
              <button
                key={ai}
                type="button"
                onClick={() => {
                  setPopoverOpen(false);
                  onAcceptAlternate(ai);
                }}
                className="flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)]"
              >
                <span>{alt.text}</span>
                <span className="font-mono text-[10px] text-[var(--color-fg-subtle)]">
                  {alt.confidence.toFixed(0)}%
                </span>
              </button>
            ))
          ) : (
            <span className="block px-2 py-1 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
              {t("editorNoAlternates")}
            </span>
          )}
          <span className="my-1 block h-px bg-[var(--color-border)]" />
          <button
            type="button"
            onClick={() => {
              setPopoverOpen(false);
              onLock();
            }}
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[var(--text-ui-sm)] text-[var(--color-success)] hover:bg-[var(--color-bg-surface)]"
          >
            <Check size={13} /> {t("editorMarkCorrect")}
          </button>
          <button
            type="button"
            onClick={() => {
              setPopoverOpen(false);
              onStartEdit();
            }}
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)]"
          >
            <Lock size={13} /> {t("editorEditManually")}
          </button>
        </span>
      )}{" "}
    </span>
  );
}

function BulkActionBar({
  count,
  contiguous,
  speakers,
  busy,
  onSelectAll,
  onClear,
  onMerge,
  onDelete,
  onSetSpeaker,
  onShiftAll,
}: {
  count: number;
  contiguous: boolean;
  speakers: Speaker[];
  busy: boolean;
  onSelectAll: () => void;
  onClear: () => void;
  onMerge: () => void;
  onDelete: () => void;
  onSetSpeaker: (speakerId: string | null) => void;
  onShiftAll: () => void;
}) {
  const t = useT();
  const [speakerMenu, setSpeakerMenu] = useState(false);

  return (
    <div className="absolute bottom-6 left-1/2 z-30 flex -translate-x-1/2 items-center gap-2 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] px-3 py-2 shadow-[var(--shadow-popover)]">
      <span className="px-1 text-[var(--text-ui-sm)] font-semibold tabular-nums">
        {t("editorSelectedCount", { n: count })}
      </span>
      <button
        type="button"
        onClick={onSelectAll}
        disabled={busy}
        className="rounded px-2 py-1 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] disabled:opacity-50"
      >
        {t("editorSelectAll")}
      </button>

      <span className="mx-1 h-5 w-px bg-[var(--color-border)]" />

      <button
        type="button"
        onClick={onMerge}
        disabled={busy || !contiguous}
        title={contiguous ? t("editorBulkMerge") : t("editorBulkMergeHint")}
        className="flex items-center gap-1.5 rounded px-2 py-1 text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)] disabled:cursor-not-allowed disabled:opacity-40"
      >
        <Merge size={14} /> {t("editorBulkMerge")}
      </button>

      <div className="relative">
        <button
          type="button"
          onClick={() => setSpeakerMenu((o) => !o)}
          disabled={busy}
          title={t("editorBulkSetSpeakerTitle")}
          className="flex items-center gap-1.5 rounded px-2 py-1 text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)] disabled:opacity-50"
        >
          <User size={14} /> {t("editorBulkSetSpeaker")}
        </button>
        {speakerMenu && (
          <div className="absolute bottom-full left-0 mb-1 w-48 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-1.5 shadow-[var(--shadow-popover)]">
            {speakers.map((s) => (
              <button
                key={s.id}
                type="button"
                onClick={() => {
                  setSpeakerMenu(false);
                  onSetSpeaker(s.id);
                }}
                className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)]"
              >
                <span
                  className="inline-block h-2 w-2 shrink-0 rounded-full"
                  style={{
                    backgroundColor: s.color_hex ?? "var(--color-accent-400)",
                  }}
                />
                {s.display_name}
              </button>
            ))}
            {speakers.length > 0 && (
              <span className="my-1 block h-px bg-[var(--color-border)]" />
            )}
            <button
              type="button"
              onClick={() => {
                setSpeakerMenu(false);
                onSetSpeaker(null);
              }}
              className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)]"
            >
              {t("editorBulkNoSpeaker")}
            </button>
          </div>
        )}
      </div>

      <button
        type="button"
        onClick={onDelete}
        disabled={busy}
        className="flex items-center gap-1.5 rounded px-2 py-1 text-[var(--text-ui-sm)] text-[var(--color-danger)] hover:bg-[var(--color-bg-surface)] disabled:opacity-50"
      >
        <Trash2 size={14} /> {t("editorBulkDelete")}
      </button>

      <span className="mx-1 h-5 w-px bg-[var(--color-border)]" />

      <button
        type="button"
        onClick={onShiftAll}
        disabled={busy}
        title={t("editorShiftAll")}
        className="flex items-center gap-1.5 rounded px-2 py-1 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)] disabled:opacity-50"
      >
        <MoveHorizontal size={14} /> {t("editorShiftAll")}
      </button>

      <button
        type="button"
        onClick={onClear}
        disabled={busy}
        title={t("editorClearSelection")}
        aria-label={t("editorClearSelection")}
        className="ml-1 grid h-6 w-6 place-items-center rounded text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)] disabled:opacity-50"
      >
        <X size={14} />
      </button>
    </div>
  );
}

function ConfidenceLegend() {
  const t = useT();
  const tiers = [
    { tier: 1, label: t("tier1") },
    { tier: 2, label: t("tier2") },
    { tier: 3, label: t("tier3") },
    { tier: 4, label: t("tier4") },
  ];
  return (
    <div className="ml-auto flex items-center gap-3 text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">
      {tiers.map((row) => (
        <span key={row.tier} className="flex items-center gap-1.5">
          <span
            className={cn(
              "word inline-block h-3 w-4 rounded",
              `word-tier-${row.tier}`,
            )}
          />
          {row.label}
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

function IconBtn({
  title,
  disabled,
  onClick,
  children,
}: {
  title: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      onClick={onClick}
      className="grid h-7 w-7 place-items-center rounded text-[var(--color-fg-muted)] hover:bg-[var(--color-bg-surface)] hover:text-[var(--color-fg)] disabled:opacity-30 disabled:hover:bg-transparent"
    >
      {children}
    </button>
  );
}
