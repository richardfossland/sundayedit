import { describe, it, expect } from "vitest";

import type { MediaItem } from "@/lib/bindings/MediaItem";
import type { Project } from "@/lib/bindings/Project";
import type { TimelineItem } from "@/lib/bindings/TimelineItem";
import type { Track } from "@/lib/bindings/Track";

import { activeVideoItem, sourceTimeSec, timelineEndMs } from "./previewMap";

// ── Minimal factories (mirror the Rust model tests' `item(...)` helper) ──────

function track(id: string, index: number, extra?: Partial<Track>): Track {
  return {
    id,
    kind: "video",
    name: id,
    index,
    enabled: true,
    locked: false,
    muted: false,
    solo: false,
    ...extra,
  };
}

function media(id: string, path: string): MediaItem {
  return {
    id,
    path,
    content_hash: id,
    kind: "video",
    duration_ms: 60_000,
    width: 1920,
    height: 1080,
    fps: 30,
    has_audio: true,
    audio_wav_path: null,
    original_filename: `${id}.mp4`,
    added_at: 0,
  };
}

function item(
  id: string,
  trackId: string,
  mediaId: string | null,
  startMs: number,
  inMs: number,
  outMs: number,
  extra?: Partial<TimelineItem>,
): TimelineItem {
  return {
    id,
    track_id: trackId,
    kind: "av",
    source_media_id: mediaId,
    in_ms: inMs,
    out_ms: outMs,
    timeline_start_ms: startMs,
    speed: 1,
    transform: {
      x: 0,
      y: 0,
      scale: 1,
      rotation_deg: 0,
      opacity: 1,
      crop: null,
    },
    effects: [],
    transition_in: null,
    text: null,
    enabled: true,
    locked: false,
    ...extra,
  };
}

function project(
  tracks: Track[],
  items: TimelineItem[],
  medias: MediaItem[],
): Project {
  return {
    media: medias,
    tracks,
    timeline_items: items,
  } as unknown as Project;
}

// ── timelineEndMs ───────────────────────────────────────────────────────────

describe("timelineEndMs", () => {
  it("adds the source span at 1× speed", () => {
    expect(timelineEndMs(item("i", "t", "m", 1000, 0, 2000))).toBe(3000);
  });

  it("compresses the span at 2× speed", () => {
    expect(
      timelineEndMs(item("i", "t", "m", 1000, 0, 2000, { speed: 2 })),
    ).toBe(2000);
  });

  it("never divides by zero on a zero speed", () => {
    // speed floored at 0.01 → span/0.01 = 200_000
    expect(timelineEndMs(item("i", "t", "m", 0, 0, 2000, { speed: 0 }))).toBe(
      200_000,
    );
  });
});

// ── activeVideoItem ─────────────────────────────────────────────────────────

describe("activeVideoItem", () => {
  it("returns null when there are no timeline items", () => {
    const p = project([track("t", 0)], [], [media("m", "/m.mp4")]);
    expect(activeVideoItem(p, 100)).toBeNull();
  });

  it("resolves a single item under the playhead", () => {
    const p = project(
      [track("t", 0)],
      [item("i", "t", "m", 0, 0, 4000)],
      [media("m", "/m.mp4")],
    );
    const hit = activeVideoItem(p, 1000);
    expect(hit?.item.id).toBe("i");
    expect(hit?.media.path).toBe("/m.mp4");
  });

  it("misses when the playhead is outside the item span", () => {
    const p = project(
      [track("t", 0)],
      [item("i", "t", "m", 1000, 0, 2000)], // spans [1000, 3000)
      [media("m", "/m.mp4")],
    );
    expect(activeVideoItem(p, 500)).toBeNull(); // before
    expect(activeVideoItem(p, 3000)).toBeNull(); // end is exclusive
    expect(activeVideoItem(p, 2999)?.item.id).toBe("i"); // just inside
  });

  it("picks the top-most track when two video clips overlap", () => {
    const p = project(
      [track("bottom", 0), track("top", 1)],
      [
        item("lo", "bottom", "m1", 0, 0, 4000),
        item("hi", "top", "m2", 0, 0, 4000),
      ],
      [media("m1", "/lo.mp4"), media("m2", "/hi.mp4")],
    );
    const hit = activeVideoItem(p, 1000);
    expect(hit?.item.id).toBe("hi");
    expect(hit?.media.path).toBe("/hi.mp4");
  });

  it("skips a disabled track and falls through to the one below", () => {
    const p = project(
      [track("bottom", 0), track("top", 1, { enabled: false })],
      [
        item("lo", "bottom", "m1", 0, 0, 4000),
        item("hi", "top", "m2", 0, 0, 4000),
      ],
      [media("m1", "/lo.mp4"), media("m2", "/hi.mp4")],
    );
    expect(activeVideoItem(p, 1000)?.item.id).toBe("lo");
  });

  it("skips a disabled item", () => {
    const p = project(
      [track("t", 0)],
      [item("i", "t", "m", 0, 0, 4000, { enabled: false })],
      [media("m", "/m.mp4")],
    );
    expect(activeVideoItem(p, 1000)).toBeNull();
  });

  it("ignores items on non-video tracks", () => {
    const p = project(
      [track("cap", 0, { kind: "caption" })],
      [item("i", "cap", "m", 0, 0, 4000)],
      [media("m", "/m.mp4")],
    );
    expect(activeVideoItem(p, 1000)).toBeNull();
  });

  it("returns null when the source media can't be resolved", () => {
    const p = project(
      [track("t", 0)],
      [item("i", "t", "missing", 0, 0, 4000)],
      [media("m", "/m.mp4")],
    );
    expect(activeVideoItem(p, 1000)).toBeNull();
  });
});

// ── sourceTimeSec ───────────────────────────────────────────────────────────

describe("sourceTimeSec", () => {
  it("equals in_ms at the item's timeline start", () => {
    const it = item("i", "t", "m", 1000, 500, 2500);
    expect(sourceTimeSec(it, 1000)).toBeCloseTo(0.5); // in_ms 500 → 0.5s
  });

  it("advances at realtime for 1× speed", () => {
    const it = item("i", "t", "m", 1000, 0, 4000);
    // 1s into the clip on the timeline → 1s into the source.
    expect(sourceTimeSec(it, 2000)).toBeCloseTo(1);
  });

  it("advances at 2× the source rate when sped up", () => {
    const it = item("i", "t", "m", 1000, 0, 4000, { speed: 2 });
    // 1s of timeline at 2× → 2s of source (offset by in_ms=0).
    expect(sourceTimeSec(it, 2000)).toBeCloseTo(2);
  });
});
