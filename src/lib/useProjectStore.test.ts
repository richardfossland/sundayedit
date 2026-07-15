import { beforeEach, describe, expect, it } from "vitest";

import {
  useProjectStore,
  selectCanUndo,
  selectCanRedo,
} from "./useProjectStore";
import { SAMPLE_PROJECT } from "./sampleProject";
import type { Project } from "./bindings";

/** Direct access to store actions/state (no React needed — it's a plain store). */
function store() {
  return useProjectStore.getState();
}

beforeEach(() => {
  useProjectStore.setState({
    project: null,
    past: [],
    future: [],
    busy: false,
    inFlight: false,
  });
});

describe("useProjectStore", () => {
  it("run() commits, capping history and clearing the redo stack", async () => {
    store().reset(SAMPLE_PROJECT);
    const v1: Project = { ...SAMPLE_PROJECT, updated_at: 1 };
    await store().run(async () => v1);

    expect(store().project).toBe(v1);
    expect(selectCanUndo(store())).toBe(true);
    expect(selectCanRedo(store())).toBe(false);
  });

  it("reset() replaces the project and clears both history stacks", async () => {
    store().reset(SAMPLE_PROJECT);
    await store().run(async () => ({ ...SAMPLE_PROJECT, updated_at: 1 }));
    store().undo(); // leaves something in `future`

    const fresh: Project = { ...SAMPLE_PROJECT, id: "other" };
    store().reset(fresh);
    expect(store().project).toBe(fresh);
    expect(store().past).toEqual([]);
    expect(store().future).toEqual([]);
  });

  it("setProject() replaces without touching history (accepts an updater)", async () => {
    store().reset(SAMPLE_PROJECT);
    await store().run(async () => ({ ...SAMPLE_PROJECT, updated_at: 1 }));
    const pastLen = store().past.length;

    store().setProject((prev) => (prev ? { ...prev, name: "renamed" } : prev));
    expect(store().project?.name).toBe("renamed");
    // Direct set is non-undoable: the undo stack is untouched.
    expect(store().past.length).toBe(pastLen);
  });

  // The exact bug this store fixes: caption edits and timeline drags used to
  // live in separate stores, so they diverged and only caption edits were
  // undoable. Now BOTH flow through one `run`/undo stack — so undoing twice
  // after a caption op then a timeline op reverts each in turn.
  it("shares ONE undo stack across caption ops and timeline ops", async () => {
    const v0 = SAMPLE_PROJECT;
    store().reset(v0);

    // 1) A caption op (CaptionEditor-style): edit the first word's text.
    const afterCaptionEdit = (p: Project): Project => ({
      ...p,
      captions: p.captions.map((c, i) =>
        i === 0
          ? {
              ...c,
              words: c.words.map((w, wi) =>
                wi === 0 ? { ...w, text: "EDITED", edited: true } : w,
              ),
            }
          : c,
      ),
    });
    await store().run(async (p) => afterCaptionEdit(p));
    const v1 = store().project!;
    expect(v1.captions[0].words[0].text).toBe("EDITED");

    // 2) A timeline op (Timeline-style): move the first caption's start.
    const afterTimelineMove = (p: Project): Project => ({
      ...p,
      captions: p.captions.map((c, i) =>
        i === 0 ? { ...c, start_ms: c.start_ms + 500 } : c,
      ),
    });
    await store().run(async (p) => afterTimelineMove(p));
    const v2 = store().project!;
    expect(v2.captions[0].start_ms).toBe(v0.captions[0].start_ms + 500);
    // The caption edit is still present on top of the timeline move.
    expect(v2.captions[0].words[0].text).toBe("EDITED");

    // Undo #1 reverts the timeline move — caption edit survives.
    store().undo();
    expect(store().project).toBe(v1);
    expect(store().project!.captions[0].start_ms).toBe(v0.captions[0].start_ms);
    expect(store().project!.captions[0].words[0].text).toBe("EDITED");

    // Undo #2 reverts the caption edit — back to the original project.
    store().undo();
    expect(store().project).toBe(v0);
    expect(store().project!.captions[0].words[0].text).toBe(
      v0.captions[0].words[0].text,
    );

    // Redo walks forward through the same shared stack.
    store().redo();
    expect(store().project).toBe(v1);
    store().redo();
    expect(store().project).toBe(v2);
  });

  it("run() ignores re-entrant calls while an op is in flight", async () => {
    store().reset(SAMPLE_PROJECT);
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));

    const first = store().run(async (p) => {
      await gate;
      return { ...p, updated_at: 1 };
    });
    // Second call arrives before the first resolves → dropped by the guard.
    await store().run(async (p) => ({ ...p, updated_at: 999 }));
    expect(store().project?.updated_at).toBe(0); // nothing committed yet

    release();
    await first;
    expect(store().project?.updated_at).toBe(1);
    expect(store().past.length).toBe(1); // only the first op committed
  });
});
