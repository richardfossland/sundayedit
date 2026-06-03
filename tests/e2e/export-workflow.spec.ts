import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { test, expect, type Page } from "@playwright/test";

import { openDemoProject } from "./fixtures/mock-backend";

// Export workflow: load the demo, open the Export modal, generate each sidecar
// format, and compare the rendered preview against a committed fixture. This
// drives the full React → ipc.ts → backend → preview-pane path; a regression in
// caption text, timing, or argument wiring shows up as a fixture mismatch.
//
// The demo project's timings are fixed (no real transcription), so the output
// is deterministic and comparable byte-for-byte.

function fixture(name: string): string {
  return readFileSync(
    fileURLToPath(new URL(`./fixtures/${name}`, import.meta.url)),
    "utf8",
  );
}

/** Open the Export modal and click a sidecar format button (matched by its
 *  leading label, since the button's accessible name includes a description).
 *  Returns the rendered preview text. */
async function exportPreview(page: Page, label: string): Promise<string> {
  await page.getByRole("button", { name: "Eksport" }).click();
  await page.getByRole("button", { name: new RegExp(`^${label}`) }).click();
  const pre = page.locator("pre");
  await expect(pre).toBeVisible();
  // textContent preserves the exact bytes (incl. CRLF); innerText would
  // normalise line endings.
  return (await pre.textContent()) ?? "";
}

test.beforeEach(async ({ page }) => {
  await openDemoProject(page);
});

test("SRT export matches the fixture byte-for-byte", async ({ page }) => {
  const srt = await exportPreview(page, "SRT");
  expect(srt).toBe(fixture("expected-export.srt"));
});

test("VTT export matches the fixture byte-for-byte", async ({ page }) => {
  const vtt = await exportPreview(page, "VTT");
  expect(vtt).toBe(fixture("expected-export.vtt"));
});

test("ASS export matches the fixture byte-for-byte", async ({ page }) => {
  const ass = await exportPreview(page, "ASS");
  expect(ass).toBe(fixture("expected-export.ass"));
});

test("JSON export is valid and preserves per-word timing + confidence", async ({
  page,
}) => {
  const raw = await exportPreview(page, "JSON");
  const doc = JSON.parse(raw);

  // Stable export contract (mirrors services/export.rs write_json).
  expect(doc.format).toBe("sundayedit-captions");
  expect(doc.version).toBe(1);
  expect(doc.language).toBe("no");
  expect(doc.captions).toHaveLength(3);

  // First caption: full text + the killer-feature per-word data survives.
  const first = doc.captions[0];
  expect(first.id).toBe("c1");
  expect(first.text).toBe("Velkommen til gudstjenesten denne søndagen morgen");
  expect(first.words[0]).toMatchObject({ text: "Velkommen", start_ms: 0 });
  // The tier-4 demo word carries its low confidence through to export.
  const kerigma = doc.captions[1].words.find(
    (w: { text: string }) => w.text === "kerigma",
  );
  expect(kerigma.confidence).toBe(38);
});

test("SRT format complies: numbered cues, comma-ms timecodes, CRLF", async ({
  page,
}) => {
  const srt = await exportPreview(page, "SRT");

  // First cue: index "1", a "HH:MM:SS,mmm --> HH:MM:SS,mmm" line, then text.
  expect(srt.startsWith("1\r\n00:00:00,000 --> 00:00:04,200\r\n")).toBe(true);
  // SRT separates lines with CRLF and cues with a blank CRLF line.
  expect(srt).toContain("\r\n\r\n");
  // The timecode uses a comma (not a dot) before milliseconds.
  expect(srt).toMatch(/\d{2}:\d{2}:\d{2},\d{3} --> \d{2}:\d{2}:\d{2},\d{3}/);
  // Three cues → indices 1, 2, 3.
  expect(srt).toContain("\r\n3\r\n");
});

test("VTT format complies: WEBVTT header, dot-ms timecodes, LF only", async ({
  page,
}) => {
  const vtt = await exportPreview(page, "VTT");

  // Must open with the signature header.
  expect(vtt.startsWith("WEBVTT\n\n")).toBe(true);
  // VTT timecodes use a dot before milliseconds.
  expect(vtt).toMatch(/\d{2}:\d{2}:\d{2}\.\d{3} --> \d{2}:\d{2}:\d{2}\.\d{3}/);
  // No carriage returns — VTT here is LF-only (unlike SRT).
  expect(vtt).not.toContain("\r");
});

test("burn-in preview renders the styled caption and reframes per preset", async ({
  page,
}) => {
  await page.getByRole("button", { name: "Eksport" }).click();

  // Pick the landscape platform preset, then reveal the burn-in preview.
  await page.getByRole("button", { name: /^YouTube/ }).click();
  await page.getByRole("button", { name: /Vis innbrenning/ }).click();

  const frame = page.getByTestId("burnin-preview-frame");
  await expect(frame).toBeVisible();
  // The frame is cropped to the platform's aspect ratio.
  await expect(frame).toHaveCSS("aspect-ratio", "1920 / 1080");

  // The caption uses the project's burn-in style (white text), validating
  // the styling before a real render.
  const caption = page.getByTestId("burnin-preview-caption");
  await expect(caption).toHaveText(
    "Velkommen til gudstjenesten denne søndagen morgen",
  );
  await expect(caption).toHaveCSS("color", "rgb(255, 255, 255)");

  // Switching presets keeps the preview open and reframes it to the new
  // aspect ratio — letting the user compare styling across platforms.
  await page.getByRole("button", { name: /^Reels/ }).click();
  await expect(page.getByTestId("burnin-preview-frame")).toHaveCSS(
    "aspect-ratio",
    "1080 / 1920",
  );
});
