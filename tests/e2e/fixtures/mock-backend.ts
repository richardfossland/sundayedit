/**
 * Browser-side Tauri backend mock for the E2E suite.
 *
 * The Playwright specs run against `vite preview` in a plain Chromium, where
 * `window.__TAURI_INTERNALS__` does not exist — so any `invoke()` (every
 * caption op + every exporter) would throw `reading 'invoke'`. The smoke +
 * onboarding specs sidestep this by never triggering an op; the editor +
 * export workflows can't.
 *
 * Rather than re-implement the whole Rust surface, this installs a *faithful*
 * mock of the handful of commands those workflows exercise. The op + exporter
 * implementations mirror `src-tauri/src/services/{operations,export}.rs`
 * exactly (split boundary = first right-word start, merge concatenates words +
 * spans first→last, edit marks `edited`; SRT `\r\n` + `,` ms, VTT `\n` + `.`
 * ms, ASS centiseconds, JSON `sundayedit-captions` v1). The point of the E2E
 * layer is the *wiring*: React → ipc.ts (camelCase args) → invoke → render the
 * result back into the DOM. A drift in caption ids, argument names, or the
 * render round-trip fails these specs even though the math is mocked.
 *
 * When real-IPC E2E lands (tauri-driver, see playwright.config.ts), these specs
 * point at the driver and this mock is dropped.
 */

import type { Page } from "@playwright/test";

/**
 * The whole mock backend, serialised into the page as one init script. It is
 * stringified by Playwright and run before any app code, so it must be a
 * self-contained function with no outer references.
 */
function backend(): void {
  type Alternate = { text: string; confidence: number };
  type Word = { text: string; start_ms: number; end_ms: number };
  type Caption = {
    id: string;
    start_ms: number;
    end_ms: number;
    words: (Word & {
      edited?: boolean;
      locked?: boolean;
      confidence: number;
      alternates?: Alternate[];
    })[];
    speaker_id: string | null;
  };
  type Project = {
    name: string;
    language: string;
    captions: Caption[];
    speakers: { id: string; display_name: string; color_hex: string | null }[];
  };

  const captionText = (c: Caption) => c.words.map((w) => w.text).join(" ");

  // ── helpers (duplicated inside the page scope — see note above) ──
  const p2 = (n: number) => String(n).padStart(2, "0");
  const p3 = (n: number) => String(n).padStart(3, "0");
  const srtTime = (ms: number) => {
    const neg = ms < 0;
    const a = Math.abs(ms);
    const h = Math.floor(a / 3_600_000);
    const m = Math.floor(a / 60_000) % 60;
    const s = Math.floor(a / 1_000) % 60;
    return `${neg ? "-" : ""}${p2(h)}:${p2(m)}:${p2(s)},${p3(a % 1_000)}`;
  };
  const vttTime = (ms: number) => {
    const a = Math.max(0, ms);
    const h = Math.floor(a / 3_600_000);
    const m = Math.floor(a / 60_000) % 60;
    const s = Math.floor(a / 1_000) % 60;
    return `${p2(h)}:${p2(m)}:${p2(s)}.${p3(a % 1_000)}`;
  };
  const assTime = (ms: number) => {
    const a = Math.max(0, ms);
    const h = Math.floor(a / 3_600_000);
    const m = Math.floor(a / 60_000) % 60;
    const s = Math.floor(a / 1_000) % 60;
    return `${h}:${p2(m)}:${p2(s)}.${p2(Math.floor((a % 1_000) / 10))}`;
  };
  const vttEscape = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  // ── exporters (mirror services/export.rs) ──
  function exportSrt(project: Project, stripEmpty: boolean): string {
    let out = "";
    let idx = 1;
    for (const c of project.captions) {
      if (stripEmpty && c.words.length === 0) continue;
      out += `${idx}\r\n${srtTime(c.start_ms)} --> ${srtTime(c.end_ms)}\r\n${captionText(c)}\r\n\r\n`;
      idx += 1;
    }
    return out;
  }
  function exportVtt(project: Project, stripEmpty: boolean): string {
    let out = "WEBVTT\n\n";
    project.captions.forEach((c, i) => {
      if (stripEmpty && c.words.length === 0) return;
      out += `${i + 1}\n${vttTime(c.start_ms)} --> ${vttTime(c.end_ms)}\n${vttEscape(captionText(c))}\n\n`;
    });
    return out;
  }
  function exportAss(project: Project): string {
    let out = "[Script Info]\n";
    out += `Title: ${project.name}\n`;
    out += "ScriptType: v4.00+\n";
    out += "\n[V4+ Styles]\n";
    out +=
      "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n";
    out += "Style: Default,Helvetica Neue,42,&H00FFFFFF\n";
    out += "\n[Events]\n";
    out +=
      "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";
    for (const c of project.captions) {
      out += `Dialogue: 0,${assTime(c.start_ms)},${assTime(c.end_ms)},Default,,0,0,0,,${captionText(c)}\n`;
    }
    return out;
  }
  function exportJson(project: Project, stripEmpty: boolean): string {
    const doc = {
      format: "sundayedit-captions",
      version: 1,
      project: project.name,
      language: project.language,
      speakers: project.speakers.map((s) => ({
        id: s.id,
        name: s.display_name,
        color: s.color_hex,
      })),
      captions: project.captions
        .filter((c) => !(stripEmpty && c.words.length === 0))
        .map((c) => ({
          id: c.id,
          start_ms: c.start_ms,
          end_ms: c.end_ms,
          text: captionText(c),
          speaker_id: c.speaker_id,
          words: c.words.map((w) => ({
            text: w.text,
            start_ms: w.start_ms,
            end_ms: w.end_ms,
            confidence: w.confidence,
          })),
        })),
    };
    return JSON.stringify(doc, null, 2);
  }

  // ── caption ops (mirror services/operations.rs) ──
  let nextId = 1;
  function findCaption(project: Project, id: string): number {
    const i = project.captions.findIndex((c) => c.id === id);
    if (i < 0) throw err(`caption ${id} not found`);
    return i;
  }
  function err(message: string) {
    return { code: "validation", message };
  }
  function splitCaption(
    project: Project,
    captionId: string,
    atWordIndex: number,
  ): Project {
    const ci = findCaption(project, captionId);
    const orig = project.captions[ci];
    if (atWordIndex === 0 || atWordIndex >= orig.words.length) {
      throw err(`split index ${atWordIndex} out of range`);
    }
    const left = orig.words.slice(0, atWordIndex);
    const right = orig.words.slice(atWordIndex);
    const boundary = right[0].start_ms;
    const leftCap = { ...orig, words: left, end_ms: boundary };
    const rightCap = {
      ...orig,
      id: `mock-${nextId++}`,
      words: right,
      start_ms: boundary,
    };
    const captions = project.captions.slice();
    captions.splice(ci, 1, leftCap, rightCap);
    return { ...project, captions };
  }
  function mergeCaptions(project: Project, captionIds: string[]): Project {
    if (captionIds.length < 2) throw err("merge needs at least 2 caption ids");
    const indices = captionIds.map((id) => findCaption(project, id)).sort();
    for (let i = 1; i < indices.length; i += 1) {
      if (indices[i] !== indices[i - 1] + 1) {
        throw err("captions are not contiguous");
      }
    }
    const first = indices[0];
    const last = indices[indices.length - 1];
    const words = project.captions
      .slice(first, last + 1)
      .flatMap((c) => c.words);
    const merged = {
      ...project.captions[first],
      end_ms: project.captions[last].end_ms,
      words,
    };
    const captions = project.captions.slice();
    captions.splice(first, last - first + 1, merged);
    return { ...project, captions };
  }
  function editWord(
    project: Project,
    captionId: string,
    wordIndex: number,
    newText: string,
  ): Project {
    const text = newText.trim();
    if (text.length === 0) throw err("word text cannot be empty");
    const ci = findCaption(project, captionId);
    const captions = project.captions.map((c, i) => {
      if (i !== ci) return c;
      const words = c.words.map((w, wi) =>
        wi === wordIndex ? { ...w, text, edited: true } : w,
      );
      return { ...c, words };
    });
    return { ...project, captions };
  }

  function lockWord(
    project: Project,
    captionId: string,
    wordIndex: number,
    locked: boolean,
  ): Project {
    const ci = findCaption(project, captionId);
    const captions = project.captions.map((c, i) => {
      if (i !== ci) return c;
      if (wordIndex >= c.words.length) {
        throw err(`word index ${wordIndex} out of range`);
      }
      const words = c.words.map((w, wi) =>
        wi === wordIndex ? { ...w, locked } : w,
      );
      return { ...c, words };
    });
    return { ...project, captions };
  }
  function acceptAlternate(
    project: Project,
    captionId: string,
    wordIndex: number,
    alternateIndex: number,
  ): Project {
    const ci = findCaption(project, captionId);
    const captions = project.captions.map((c, i) => {
      if (i !== ci) return c;
      if (wordIndex >= c.words.length) {
        throw err(`word index ${wordIndex} out of range`);
      }
      const alt = (c.words[wordIndex].alternates ?? [])[alternateIndex];
      if (!alt) throw err(`alternate index ${alternateIndex} out of range`);
      const words = c.words.map((w, wi) =>
        wi === wordIndex
          ? { ...w, text: alt.text, confidence: alt.confidence, edited: true }
          : w,
      );
      return { ...c, words };
    });
    return { ...project, captions };
  }
  function retimeWord(
    project: Project,
    captionId: string,
    wordIndex: number,
    newStartMs: number,
    newEndMs: number,
  ): Project {
    if (newStartMs >= newEndMs) throw err("start must be less than end");
    const ci = findCaption(project, captionId);
    const cap = project.captions[ci];
    if (wordIndex >= cap.words.length) {
      throw err(`word index ${wordIndex} out of range`);
    }
    // Bounds vs caption + neighbours (mirrors operations.rs::retime_word).
    const lower =
      wordIndex === 0 ? cap.start_ms : cap.words[wordIndex - 1].end_ms;
    const upper =
      wordIndex + 1 >= cap.words.length
        ? cap.end_ms
        : cap.words[wordIndex + 1].start_ms;
    if (newStartMs < lower || newEndMs > upper) {
      throw err(`retime (${newStartMs}, ${newEndMs}) outside bounds`);
    }
    const captions = project.captions.map((c, i) => {
      if (i !== ci) return c;
      const words = c.words.map((w, wi) =>
        wi === wordIndex ? { ...w, start_ms: newStartMs, end_ms: newEndMs } : w,
      );
      return { ...c, words };
    });
    return { ...project, captions };
  }
  function moveCaption(
    project: Project,
    captionId: string,
    deltaMs: number,
  ): Project {
    if (deltaMs === 0) return project;
    const idx = findCaption(project, captionId);
    const cap = project.captions[idx];
    const prevEnd = idx > 0 ? project.captions[idx - 1].end_ms : 0;
    const nextStart = project.captions[idx + 1]?.start_ms;
    const dur = cap.end_ms - cap.start_ms;
    const lo = Math.max(prevEnd, 0);
    const hi = nextStart === undefined ? Infinity : nextStart - dur;
    // Clamp the slide into the gap, NLE-style (mirrors operations.rs).
    const clamped =
      hi < lo
        ? cap.start_ms
        : Math.min(Math.max(cap.start_ms + deltaMs, lo), hi);
    const applied = clamped - cap.start_ms;
    if (applied === 0) return project;
    const captions = project.captions.map((c, i) => {
      if (i !== idx) return c;
      return {
        ...c,
        start_ms: c.start_ms + applied,
        end_ms: c.end_ms + applied,
        words: c.words.map((w) => ({
          ...w,
          start_ms: w.start_ms + applied,
          end_ms: w.end_ms + applied,
        })),
      };
    });
    return { ...project, captions };
  }
  function resizeCaption(
    project: Project,
    captionId: string,
    newStartMs: number,
    newEndMs: number,
  ): Project {
    if (newStartMs >= newEndMs) throw err("start must be less than end");
    const idx = findCaption(project, captionId);
    const cap = project.captions[idx];
    const prevEnd = idx > 0 ? project.captions[idx - 1].end_ms : 0;
    const nextStart = project.captions[idx + 1]?.start_ms;
    const wordsLo = cap.words[0]?.start_ms;
    const wordsHi = cap.words[cap.words.length - 1]?.end_ms;
    // Start edge: clamped to prev caption end / 0, can't pass first word start.
    let start = Math.max(newStartMs, prevEnd, 0);
    if (wordsLo !== undefined) start = Math.min(start, wordsLo);
    // End edge: clamped to next caption start, can't shrink past last word end.
    let end = newEndMs;
    if (nextStart !== undefined) end = Math.min(end, nextStart);
    if (wordsHi !== undefined) end = Math.max(end, wordsHi);
    if (start >= end) throw err("resize leaves the caption with no duration");
    const captions = project.captions.map((c, i) =>
      i === idx ? { ...c, start_ms: start, end_ms: end } : c,
    );
    return { ...project, captions };
  }
  function shiftAllCaptions(project: Project, offsetMs: number): Project {
    if (offsetMs === 0) return project;
    const captions = project.captions.map((c) => ({
      ...c,
      start_ms: Math.max(c.start_ms + offsetMs, 0),
      end_ms: Math.max(c.end_ms + offsetMs, 0),
      words: c.words.map((w) => ({
        ...w,
        start_ms: Math.max(w.start_ms + offsetMs, 0),
        end_ms: Math.max(w.end_ms + offsetMs, 0),
      })),
    }));
    return { ...project, captions };
  }

  type Args = Record<string, unknown>;
  function invoke(cmd: string, args: Args): Promise<unknown> {
    const project = args.project as Project;
    switch (cmd) {
      case "op_split_caption":
        return Promise.resolve(
          splitCaption(
            project,
            args.captionId as string,
            args.atWordIndex as number,
          ),
        );
      case "op_merge_captions":
        return Promise.resolve(
          mergeCaptions(project, args.captionIds as string[]),
        );
      case "op_edit_word":
        return Promise.resolve(
          editWord(
            project,
            args.captionId as string,
            args.wordIndex as number,
            args.newText as string,
          ),
        );
      case "op_lock_word":
        return Promise.resolve(
          lockWord(
            project,
            args.captionId as string,
            args.wordIndex as number,
            args.locked as boolean,
          ),
        );
      case "op_accept_alternate":
        return Promise.resolve(
          acceptAlternate(
            project,
            args.captionId as string,
            args.wordIndex as number,
            args.alternateIndex as number,
          ),
        );
      case "op_retime_word":
        return Promise.resolve(
          retimeWord(
            project,
            args.captionId as string,
            args.wordIndex as number,
            args.newStartMs as number,
            args.newEndMs as number,
          ),
        );
      case "op_move_caption":
        return Promise.resolve(
          moveCaption(
            project,
            args.captionId as string,
            args.deltaMs as number,
          ),
        );
      case "op_resize_caption":
        return Promise.resolve(
          resizeCaption(
            project,
            args.captionId as string,
            args.newStartMs as number,
            args.newEndMs as number,
          ),
        );
      case "op_shift_all_captions":
        return Promise.resolve(
          shiftAllCaptions(project, args.offsetMs as number),
        );
      case "op_apply_glossary":
        return Promise.resolve({ project, corrections: [] });
      case "export_srt":
        return Promise.resolve(exportSrt(project, args.stripEmpty !== false));
      case "export_vtt":
        return Promise.resolve(exportVtt(project, args.stripEmpty !== false));
      case "export_ass":
        return Promise.resolve(exportAss(project));
      case "export_json":
        return Promise.resolve(exportJson(project, args.stripEmpty !== false));
      case "export_txt":
        return Promise.resolve(
          project.captions.map(captionText).join(" ").trim(),
        );
      case "export_list_presets":
        return Promise.resolve([]);
      case "export_validate":
        return Promise.resolve([]);
      default:
        // Unhandled commands (downloaded models, deep-link, updater) resolve
        // empty so app boot doesn't throw — the workflows don't depend on them.
        return Promise.resolve(undefined);
    }
  }

  const w = window as unknown as { __TAURI_INTERNALS__: unknown };
  w.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: unknown) => cb,
    convertFileSrc: (path: string) => path,
    unregisterCallback: () => {},
  };
}

/**
 * Install the mock backend + a deterministic locale/onboarding state, then
 * load the app and click into the bundled demo project. Leaves the page in the
 * editor shell, ready for workflow assertions.
 */
export async function openDemoProject(page: Page): Promise<void> {
  await page.addInitScript(backend);
  await page.addInitScript(() => {
    localStorage.setItem("sundayedit.onboarded", "1");
    localStorage.setItem("sundayedit.locale", "no");
  });
  await page.goto("/");
  await page.getByRole("button", { name: /utforsk demo-prosjektet/i }).click();
  // The editor heading confirms we're in the shell, not the import screen.
  await page
    .getByRole("heading", { name: "Editor" })
    .waitFor({ state: "visible" });
}
