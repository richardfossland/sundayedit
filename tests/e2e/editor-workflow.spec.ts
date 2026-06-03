import { test, expect, type Page } from "@playwright/test";

import { openDemoProject } from "./fixtures/mock-backend";

// Editor workflow: load the bundled demo, then exercise the core editing
// surface end-to-end (React → ipc.ts → mock backend → re-render). The smoke +
// onboarding specs only verify routing; these drive real caption operations.
//
// The demo's three captions ("Velkommen til …", "I dag skal …", "La oss be …")
// are hand-picked so all four confidence tiers are present — see
// src/lib/sampleProject.ts.

test.beforeEach(async ({ page }) => {
  await openDemoProject(page);
});

test("demo captions render with all four confidence tiers highlighted", async ({
  page,
}) => {
  // Every word becomes a `.word` span carrying its `.word-tier-N` class.
  await expect(page.locator(".word").first()).toBeVisible();

  // All four tiers appear at least once (confident → very unsure). The demo is
  // calibrated so each tier is represented; a regression in the tier mapping
  // (bindings.confidenceTier) or the threshold logic would drop one.
  for (const tier of [1, 2, 3, 4]) {
    await expect(page.locator(`.word-tier-${tier}`).first()).toBeVisible();
  }

  // The first caption's words render in order.
  await expect(page.getByRole("button", { name: "Velkommen" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "gudstjenesten" }),
  ).toBeVisible();
});

test("inline word edit commits through IPC and persists in the DOM", async ({
  page,
}) => {
  // Click the low-confidence "kerigma" (the plan's demo case) to edit it.
  const target = page.getByRole("button", { name: "kerigma" });
  await expect(target).toBeVisible();
  await target.click();

  // An inline <input> appears, auto-focused and seeded with the current text.
  // (Other inputs exist — the threshold slider, the open glossary panel — so we
  // target the focused edit field rather than a positional locator.)
  const input = page.locator("input:focus");
  await expect(input).toHaveValue("kerigma");
  await input.fill("kerygma");
  await input.press("Enter");

  // The op round-trips through the mock (op_edit_word) and re-renders: the new
  // text is present, the old is gone, and the word is now marked edited (tier
  // 1, no highlight) — confidence highlighting stops on a human-corrected word.
  await expect(page.getByRole("button", { name: "kerygma" })).toBeVisible();
  await expect(page.getByRole("button", { name: "kerigma" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "kerygma" })).toHaveClass(
    /is-edited/,
  );
});

test("editing an uncertain word reduces the uncertain-word count", async ({
  page,
}) => {
  // The review progress line reads "<n> usikre ord av <total>".
  await expect(page.getByText(/usikre ord av/)).toBeVisible();

  // "be" (confidence 55) is below the default threshold (70) → uncertain.
  // Editing it marks it edited, dropping it out of the uncertain set.
  const before = await readUncertainCount(page);

  const be = page.getByRole("button", { name: "be", exact: true });
  await be.click();
  const input = page.locator("input:focus");
  await expect(input).toHaveValue("be");
  await input.fill("bes");
  await input.press("Enter");
  await expect(page.getByRole("button", { name: "bes" })).toBeVisible();

  expect(await readUncertainCount(page)).toBeLessThan(before);
});

// Split + merge have no toolbar trigger in the current editor (they're ops the
// timeline will surface later). We still guard their IPC contract — the exact
// command name + camelCase args ipc.ts sends — so a caption-id / argument-name
// regression fails here, which is precisely the integration bug unit tests in
// isolation miss.

test("split-caption op splits at a word boundary into two captions", async ({
  page,
}) => {
  const result = await splitDemoCaption(page, "c2", 4);
  // c2 ("I dag skal vi snakke om kerigma og frelse") splits before "snakke":
  // the 2-caption input becomes 3, the left keeps id c2, the right is fresh.
  expect(result.count).toBe(3);
  expect(result.leftId).toBe("c2");
  expect(result.rightFirstWord).toBe("snakke");
  // The boundary timing is the split word's start (5500ms) — no corruption.
  expect(result.leftEnd).toBe(5500);
  expect(result.rightStart).toBe(5500);
});

test("merge-captions op joins adjacent captions and concatenates words", async ({
  page,
}) => {
  const result = await mergeDemoCaptions(page, ["c1", "c2"]);
  // c1 + c2 → one caption keeping c1's id, spanning c1.start → c2.end.
  expect(result.count).toBe(1);
  expect(result.mergedId).toBe("c1");
  expect(result.start).toBe(0); // c1 start
  expect(result.end).toBe(9800); // c2 end
  expect(result.text).toBe(
    "Velkommen til gudstjenesten denne søndagen morgen I dag skal vi snakke om kerigma og frelse",
  );
});

async function readUncertainCount(page: Page) {
  // The count sits in the first <strong> right before the "usikre ord av" copy.
  const text = await page.locator("strong").first().innerText();
  return Number(text.trim());
}

// ── IPC-contract drivers ────────────────────────────────────────────────────
// `vite preview` serves the built bundle, not raw TS source, so the demo
// project isn't importable in-page. We rebuild the two captions these ops need
// inline (mirroring src/lib/sampleProject.ts) and call the backend exactly as
// ipc.ts does — same command names + camelCase args. A drift in either fails.

type DemoWord = { text: string; start_ms: number; end_ms: number };
type DemoCaption = {
  id: string;
  start_ms: number;
  end_ms: number;
  words: DemoWord[];
  speaker_id: string | null;
};

function word(text: string, start: number, end: number): DemoWord {
  return { text, start_ms: start, end_ms: end };
}

// c1 + c2 from sampleProject.ts — the timing the boundary/merge assertions rely
// on. Confidence is irrelevant to split/merge so it's omitted here.
const DEMO_C1: DemoCaption = {
  id: "c1",
  start_ms: 0,
  end_ms: 4200,
  speaker_id: "s1",
  words: [
    word("Velkommen", 0, 700),
    word("til", 700, 900),
    word("gudstjenesten", 900, 1800),
    word("denne", 1800, 2100),
    word("søndagen", 2100, 2900),
    word("morgen", 2900, 4200),
  ],
};
const DEMO_C2: DemoCaption = {
  id: "c2",
  start_ms: 4500,
  end_ms: 9800,
  speaker_id: "s1",
  words: [
    word("I", 4500, 4700),
    word("dag", 4700, 5000),
    word("skal", 5000, 5300),
    word("vi", 5300, 5500),
    word("snakke", 5500, 6000),
    word("om", 6000, 6200),
    word("kerigma", 6200, 7100),
    word("og", 7100, 7300),
    word("frelse", 7300, 9800),
  ],
};

function demoProject(captions: DemoCaption[]) {
  return { name: "demo", language: "no", speakers: [], captions };
}

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

async function splitDemoCaption(page: Page, captionId: string, atWord: number) {
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_split_caption",
    {
      project: demoProject([DEMO_C1, DEMO_C2]),
      captionId,
      atWordIndex: atWord,
    },
  );
  const idx = next.captions.findIndex((c) => c.id === captionId);
  const right = next.captions[idx + 1];
  return {
    count: next.captions.length,
    leftId: next.captions[idx].id,
    leftEnd: next.captions[idx].end_ms,
    rightStart: right.start_ms,
    rightFirstWord: right.words[0].text,
  };
}

async function mergeDemoCaptions(page: Page, ids: string[]) {
  const next = await invoke<{ captions: DemoCaption[] }>(
    page,
    "op_merge_captions",
    { project: demoProject([DEMO_C1, DEMO_C2]), captionIds: ids },
  );
  const merged = next.captions.find((c) => c.id === ids[0])!;
  return {
    count: next.captions.length,
    mergedId: merged.id,
    start: merged.start_ms,
    end: merged.end_ms,
    text: merged.words.map((w) => w.text).join(" "),
  };
}
