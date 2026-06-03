import { test, expect, type Page } from "@playwright/test";

import { openDemoProject } from "./fixtures/mock-backend";

// Timeline-critical caption operations, end-to-end.
//
// Six ops drive the timeline UI (phase 1.3) but have no toolbar trigger yet —
// they're surfaced by the timeline canvas (lock/accept-alternate from the word
// inspector; move/resize/retime from drag handles; shift-all from the sync
// nudge). Like split + merge in editor-workflow.spec.ts we can't click them, so
// we guard their IPC contract directly: the exact command name + camelCase args
// ipc.ts sends, plus the semantic round-trip (lock removes the highlight,
// retime preserves boundaries, move/resize preserve caption order…). A drift in
// any caption-id / argument-name — the integration bug Rust unit tests miss —
// fails here. The op math mirrors services/operations.rs and is asserted there.

test.beforeEach(async ({ page }) => {
  await openDemoProject(page);
});

// ── lockWord ──────────────────────────────────────────────────────────────────

test("lock-word op confirms a word, removing its confidence highlight", async ({
  page,
}) => {
  // c1 word 5 ("morgen") is tier 3 (confidence 64). Locking it confirms the
  // word: locked words render tier 1 — no highlight — regardless of score.
  const next = await invoke<{ captions: DemoCaption[] }>(page, "op_lock_word", {
    project: demoProject([DEMO_C1, DEMO_C2]),
    captionId: "c1",
    wordIndex: 5,
    locked: true,
  });
  const word = next.captions[0].words[5];
  expect(word.locked).toBe(true);
  expect(word.text).toBe("morgen"); // text untouched
  expect(word.confidence).toBe(64); // score untouched — lock only hides it
});

test("lock-word op is a toggle — re-running with locked:false clears it", async ({
  page,
}) => {
  const locked = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_lock_word",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      wordIndex: 5,
      locked: true,
    },
  );
  expect(locked.captions[0].words[5].locked).toBe(true);
  const unlocked = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_lock_word",
    { project: locked, captionId: "c1", wordIndex: 5, locked: false },
  );
  expect(unlocked.captions[0].words[5].locked).toBe(false);
});

// ── acceptAlternate ─────────────────────────────────────────────────────────

test("accept-alternate op swaps in the ASR alternate's text + confidence", async ({
  page,
}) => {
  // c2 word 6 ("kerigma", tier 4) carries alternates from the ASR pass; the
  // plan's demo case fixes it to "kerygma" by accepting alternate 0.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_accept_alternate",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c2",
      wordIndex: 6,
      alternateIndex: 0,
    },
  );
  const word = next.captions[1].words[6];
  expect(word.text).toBe("kerygma"); // alternate 0's text
  expect(word.confidence).toBe(71); // alternate 0's confidence
  expect(word.edited).toBe(true); // marked edited → drops the highlight
});

// ── retimeWord ──────────────────────────────────────────────────────────────

test("retime-word op moves a word's timing within its caption bounds", async ({
  page,
}) => {
  // c1 word 1 ("til") is 700..900; word 0 ends at 700, word 2 starts at 900.
  // Retiming to 720..880 stays inside the neighbour window.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_retime_word",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      wordIndex: 1,
      newStartMs: 720,
      newEndMs: 880,
    },
  );
  const word = next.captions[0].words[1];
  expect(word.start_ms).toBe(720);
  expect(word.end_ms).toBe(880);
  // Neighbours are untouched — retime preserves the surrounding boundaries.
  expect(next.captions[0].words[0].end_ms).toBe(700);
  expect(next.captions[0].words[2].start_ms).toBe(900);
});

test("retime-word op rejects a range that crosses a neighbour", async ({
  page,
}) => {
  // word 0 ("Velkommen") ends at 700; pushing word 1 to start at 600 encroaches
  // on it → validation error (mirrors operations.rs::retime_word bounds check).
  const error = await invokeError(page, "op_retime_word", {
    project: demoProject([DEMO_C1, DEMO_C2]),
    captionId: "c1",
    wordIndex: 1,
    newStartMs: 600,
    newEndMs: 880,
  });
  expect(error.code).toBe("validation");
});

// ── moveCaption ─────────────────────────────────────────────────────────────

test("move-caption op slides a caption and its words together", async ({
  page,
}) => {
  // c1 [0,4200] has room to slide right: c2 starts at 4500, dur 4200 → max
  // start 300. A +200 drag moves the box and every word by exactly 200.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_move_caption",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      deltaMs: 200,
    },
  );
  const c1 = next.captions[0];
  expect(c1.start_ms).toBe(200);
  expect(c1.end_ms).toBe(4400);
  expect(c1.words[0].start_ms).toBe(200); // first word slid with the box
  expect(c1.words[5].end_ms).toBe(4400); // last word too
  // Caption order is preserved — c1 still first, c2 still second.
  expect(next.captions.map((c) => c.id)).toEqual(["c1", "c2"]);
});

test("move-caption op clamps the slide at the next caption", async ({
  page,
}) => {
  // A huge drag can't push c1 past c2: it stops at c2.start (4500) − dur (4200).
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_move_caption",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      deltaMs: 99_999,
    },
  );
  const c1 = next.captions[0];
  expect(c1.start_ms).toBe(300); // 4500 − 4200
  expect(c1.end_ms).toBe(4500); // butts up against c2, never overlaps
  expect(next.captions.map((c) => c.id)).toEqual(["c1", "c2"]);
});

// ── resizeCaption ─────────────────────────────────────────────────────────────

test("resize-caption op extends the end edge into the gap", async ({
  page,
}) => {
  // c1 ends at 4200; its last word ends at 4200; c2 starts at 4500. Dragging
  // the end edge to 4400 fits in the gap. Words are left untouched.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_resize_caption",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      newStartMs: 0,
      newEndMs: 4400,
    },
  );
  expect(next.captions[0].end_ms).toBe(4400);
  expect(next.captions[0].words[5].end_ms).toBe(4200); // word untouched
  expect(next.captions.map((c) => c.id)).toEqual(["c1", "c2"]);
});

test("resize-caption op clamps the end edge at the next caption", async ({
  page,
}) => {
  // Dragging c1's end past c2.start (4500) clamps to 4500 — captions never
  // overlap, order preserved.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_resize_caption",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId: "c1",
      newStartMs: 0,
      newEndMs: 6000,
    },
  );
  expect(next.captions[0].end_ms).toBe(4500);
  expect(next.captions.map((c) => c.id)).toEqual(["c1", "c2"]);
});

// ── shiftAllCaptions ──────────────────────────────────────────────────────────

test("shift-all-captions op nudges every caption + word by the offset", async ({
  page,
}) => {
  // The sync nudge fixes a constant audio/Whisper offset: +1000 pushes the
  // whole project 1s later, in lockstep.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_shift_all_captions",
    { project: demoProject([DEMO_C1, DEMO_C2]), offsetMs: 1000 },
  );
  expect(next.captions[0].start_ms).toBe(1000); // 0 + 1000
  expect(next.captions[0].words[0].start_ms).toBe(1000);
  expect(next.captions[1].start_ms).toBe(5500); // 4500 + 1000
  expect(next.captions[1].end_ms).toBe(10_800); // 9800 + 1000
  // Order is preserved — a uniform shift can't reorder.
  expect(next.captions.map((c) => c.id)).toEqual(["c1", "c2"]);
});

test("shift-all-captions op floors negative times at zero", async ({
  page,
}) => {
  // c1 starts at 0; shifting −500 can't go negative — start/end clamp at 0
  // (mirrors operations.rs `.max(0)`). c2 has headroom so it shifts cleanly.
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_shift_all_captions",
    { project: demoProject([DEMO_C1, DEMO_C2]), offsetMs: -500 },
  );
  expect(next.captions[0].start_ms).toBe(0); // floored
  expect(next.captions[1].start_ms).toBe(4000); // 4500 − 500, no floor
});

// ── IPC-contract drivers ────────────────────────────────────────────────────
// `vite preview` serves the built bundle, not raw TS source, so the demo
// project isn't importable in-page. We rebuild the captions these ops need
// inline (mirroring src/lib/sampleProject.ts) and call the backend exactly as
// ipc.ts does — same command names + camelCase args. A drift in either fails.
// Unlike split/merge, lock + accept-alternate care about per-word confidence +
// alternates, so those fields are carried here too.

type DemoAlternate = { text: string; confidence: number };
type DemoWord = {
  text: string;
  start_ms: number;
  end_ms: number;
  confidence: number;
  alternates?: DemoAlternate[];
};
type DemoCaption = {
  id: string;
  start_ms: number;
  end_ms: number;
  words: (DemoWord & { edited?: boolean; locked?: boolean })[];
  speaker_id: string | null;
};

function word(
  text: string,
  start: number,
  end: number,
  confidence: number,
  alternates?: DemoAlternate[],
): DemoWord {
  return alternates
    ? { text, start_ms: start, end_ms: end, confidence, alternates }
    : { text, start_ms: start, end_ms: end, confidence };
}

// c1 + c2 from sampleProject.ts, with the confidence + alternates the lock /
// accept-alternate / shift assertions rely on.
const DEMO_C1: DemoCaption = {
  id: "c1",
  start_ms: 0,
  end_ms: 4200,
  speaker_id: "s1",
  words: [
    word("Velkommen", 0, 700, 96),
    word("til", 700, 900, 98),
    word("gudstjenesten", 900, 1800, 91),
    word("denne", 1800, 2100, 88),
    word("søndagen", 2100, 2900, 72),
    word("morgen", 2900, 4200, 64),
  ],
};
const DEMO_C2: DemoCaption = {
  id: "c2",
  start_ms: 4500,
  end_ms: 9800,
  speaker_id: "s1",
  words: [
    word("I", 4500, 4700, 97),
    word("dag", 4700, 5000, 95),
    word("skal", 5000, 5300, 94),
    word("vi", 5300, 5500, 96),
    word("snakke", 5500, 6000, 90),
    word("om", 6000, 6200, 97),
    word("kerigma", 6200, 7100, 38, [
      { text: "kerygma", confidence: 71 },
      { text: "karisma", confidence: 44 },
      { text: "kerigma", confidence: 38 },
    ]),
    word("og", 7100, 7300, 98),
    word("frelse", 7300, 9800, 86),
  ],
};

function demoProject(captions: DemoCaption[]) {
  return { name: "demo", language: "no", speakers: [], captions };
}

// Resolve path: call the mock backend exactly as ipc.ts's `call` does.
async function invoke<T>(
  page: Page,
  cmd: string,
  args: Record<string, unknown>,
): Promise<T> {
  return page.evaluate(
    ({ cmd, args }) => {
      const w = window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (c: string, a: Record<string, unknown>) => Promise<unknown>;
        };
      };
      return w.__TAURI_INTERNALS__.invoke(cmd, args) as Promise<unknown>;
    },
    { cmd, args },
  ) as Promise<T>;
}

// Reject path: capture the structured `{ code, message }` the op throws so the
// validation contract (not just the happy path) is guarded.
async function invokeError(
  page: Page,
  cmd: string,
  args: Record<string, unknown>,
): Promise<{ code: string; message: string }> {
  return page.evaluate(
    async ({ cmd, args }) => {
      const w = window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (c: string, a: Record<string, unknown>) => Promise<unknown>;
        };
      };
      try {
        await w.__TAURI_INTERNALS__.invoke(cmd, args);
        return { code: "none", message: "expected the op to reject" };
      } catch (e) {
        return e as { code: string; message: string };
      }
    },
    { cmd, args },
  );
}
