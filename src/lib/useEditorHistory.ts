/**
 * Editor history — Phase 3.2 undo/redo.
 *
 * Because every Rust caption operation is a pure function returning a new
 * Project (see ADR-002), undo/redo is trivial: keep a stack of previous
 * Project states. We never diff; we snapshot.
 *
 * Usage:
 *   const editor = useEditorHistory(initialProject);
 *   await editor.run(p => ipc.ops.editWord(p, capId, i, "new"));
 *   editor.undo();  editor.redo();
 *
 * `run` takes an async operation that maps the current Project to a new
 * one (via the Rust IPC). On success it commits: pushes the old state to
 * `past`, clears `future`. On failure it leaves state untouched and
 * rethrows so the caller can surface the error.
 *
 * History is capped (default 100) so long sessions don't grow unbounded.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { Project } from "./bindings";

const HISTORY_CAP = 100;

export interface EditorHistory {
  project: Project;
  canUndo: boolean;
  canRedo: boolean;
  /** Run an async op (current Project → new Project). Commits on success. */
  run: (op: (current: Project) => Promise<Project>) => Promise<void>;
  /** Replace the whole project (e.g. after import/open). Resets history. */
  reset: (project: Project) => void;
  undo: () => void;
  redo: () => void;
  /** Whether an op is currently in flight (disable buttons). */
  busy: boolean;
}

export function useEditorHistory(initial: Project): EditorHistory {
  const [project, setProject] = useState<Project>(initial);
  const [past, setPast] = useState<Project[]>([]);
  const [future, setFuture] = useState<Project[]>([]);
  const [busy, setBusy] = useState(false);
  // Guard against overlapping ops corrupting the stacks.
  const inFlight = useRef(false);

  const run = useCallback(
    async (op: (current: Project) => Promise<Project>) => {
      if (inFlight.current) return;
      inFlight.current = true;
      setBusy(true);
      try {
        const next = await op(project);
        setPast((p) => {
          const appended = [...p, project];
          return appended.length > HISTORY_CAP
            ? appended.slice(appended.length - HISTORY_CAP)
            : appended;
        });
        setFuture([]);
        setProject(next);
      } finally {
        inFlight.current = false;
        setBusy(false);
      }
    },
    [project],
  );

  const undo = useCallback(() => {
    setPast((p) => {
      if (p.length === 0) return p;
      const previous = p[p.length - 1];
      setFuture((f) => [project, ...f]);
      setProject(previous);
      return p.slice(0, -1);
    });
  }, [project]);

  const redo = useCallback(() => {
    setFuture((f) => {
      if (f.length === 0) return f;
      const next = f[0];
      setPast((p) => [...p, project]);
      setProject(next);
      return f.slice(1);
    });
  }, [project]);

  const reset = useCallback((p: Project) => {
    setProject(p);
    setPast([]);
    setFuture([]);
  }, []);

  // ⌘Z / ⌘⇧Z (Ctrl on Windows). Ignore while typing in an input.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod || e.key.toLowerCase() !== "z") return;
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.tagName === "INPUT" ||
          t.tagName === "TEXTAREA" ||
          t.isContentEditable)
      ) {
        return; // let the field handle its own undo
      }
      e.preventDefault();
      if (e.shiftKey) redo();
      else undo();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [undo, redo]);

  return {
    project,
    canUndo: past.length > 0,
    canRedo: future.length > 0,
    run,
    reset,
    undo,
    redo,
    busy,
  };
}
