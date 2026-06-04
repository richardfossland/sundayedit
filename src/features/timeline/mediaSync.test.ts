import { describe, it, expect } from "vitest";

import {
  snapToFrameSec,
  intentFor,
  reconcile,
  isUserGesture,
  type VideoActual,
} from "./mediaSync";

describe("snapToFrameSec", () => {
  it("rounds to the nearest frame boundary", () => {
    // 30fps → frames every 1/30 s. 0.5s = frame 15 exactly.
    expect(snapToFrameSec(0.5, 30)).toBeCloseTo(0.5);
    // 0.52s ≈ frame 15.6 → 16/30 ≈ 0.5333
    expect(snapToFrameSec(0.52, 30)).toBeCloseTo(16 / 30);
  });

  it("is a no-op for non-positive fps", () => {
    expect(snapToFrameSec(1.234, 0)).toBe(1.234);
    expect(snapToFrameSec(1.234, -5)).toBe(1.234);
  });
});

describe("intentFor", () => {
  it("converts ms → frame-snapped seconds", () => {
    const i = intentFor(500, 1, 60, 30);
    expect(i.timeSec).toBeCloseTo(0.5);
  });

  it("clamps the target to [0, duration]", () => {
    expect(intentFor(-2000, 1, 60, 30).timeSec).toBe(0);
    expect(intentFor(999_000, 1, 60, 30).timeSec).toBeCloseTo(60);
  });

  it("plays only at realtime forward rate", () => {
    expect(intentFor(1000, 1, 60, 30).shouldPlay).toBe(true);
    expect(intentFor(1000, 0, 60, 30).shouldPlay).toBe(false);
    // Reverse and fast shuttle scrub by seeking, so the element stays paused.
    expect(intentFor(1000, -1, 60, 30).shouldPlay).toBe(false);
    expect(intentFor(1000, 4, 60, 30).shouldPlay).toBe(false);
  });

  it("does not play once the playhead reaches the end", () => {
    expect(intentFor(60_000, 1, 60, 30).shouldPlay).toBe(false);
  });

  it("decides shouldPlay against the timeline duration, clamps seek to the element", () => {
    // Probe metadata (timeline authority) says 60s but the real container is
    // 59.5s. The timeline clock stops at 60s, so the preview must keep
    // "playing" up to the timeline end rather than pausing early at the
    // element's 59.5s. The seek target is still clamped to real footage.
    const i = intentFor(59_600, 1, 60, 30, 59.5);
    expect(i.shouldPlay).toBe(true);
    expect(i.timeSec).toBeLessThanOrEqual(59.5);
  });

  it("stops at the timeline end, not the element end, when the element is longer", () => {
    // Element reports 60.5s but the timeline (export domain) is 60s.
    expect(intentFor(60_000, 1, 60, 30, 60.5).shouldPlay).toBe(false);
  });

  it("always reports a positive native playbackRate (HTML can't reverse)", () => {
    expect(intentFor(1000, -1, 60, 30).playbackRate).toBeGreaterThan(0);
    expect(intentFor(1000, 1, 60, 30).playbackRate).toBeGreaterThan(0);
  });
});

describe("reconcile", () => {
  const actual = (over: Partial<VideoActual>): VideoActual => ({
    currentTimeSec: 0,
    paused: true,
    durationSec: 60,
    ...over,
  });

  it("issues play when the timeline wants playback and the element is paused", () => {
    const step = reconcile(
      { timeSec: 1, shouldPlay: true, playbackRate: 1 },
      actual({ paused: true, currentTimeSec: 1 }),
    );
    expect(step.transport).toBe("play");
  });

  it("issues pause when the timeline stops and the element is playing", () => {
    const step = reconcile(
      { timeSec: 1, shouldPlay: false, playbackRate: 1 },
      actual({ paused: false, currentTimeSec: 1 }),
    );
    expect(step.transport).toBe("pause");
  });

  it("issues no transport when already in the wanted state", () => {
    expect(
      reconcile(
        { timeSec: 1, shouldPlay: true, playbackRate: 1 },
        actual({ paused: false, currentTimeSec: 1 }),
      ).transport,
    ).toBeNull();
    expect(
      reconcile(
        { timeSec: 1, shouldPlay: false, playbackRate: 1 },
        actual({ paused: true, currentTimeSec: 1 }),
      ).transport,
    ).toBeNull();
  });

  it("pins currentTime to the playhead while paused (scrub/reverse)", () => {
    const step = reconcile(
      { timeSec: 2.5, shouldPlay: false, playbackRate: 1 },
      actual({ paused: true, currentTimeSec: 1.0 }),
    );
    expect(step.seekTo).toBeCloseTo(2.5);
  });

  it("does not chase small drift while playing (lets the element's clock run)", () => {
    // ~half a frame of drift while playing → no seek.
    const step = reconcile(
      { timeSec: 1.0, shouldPlay: true, playbackRate: 1 },
      actual({ paused: false, currentTimeSec: 1.0 + 1 / 60 }),
    );
    expect(step.seekTo).toBeNull();
  });

  it("re-seeks while playing once drift exceeds a frame", () => {
    const step = reconcile(
      { timeSec: 5.0, shouldPlay: true, playbackRate: 1 },
      actual({ paused: false, currentTimeSec: 1.0 }),
    );
    expect(step.seekTo).toBeCloseTo(5.0);
  });

  it("does not re-seek a paused element already at the target", () => {
    const step = reconcile(
      { timeSec: 2.0, shouldPlay: false, playbackRate: 1 },
      actual({ paused: true, currentTimeSec: 2.0 }),
    );
    expect(step.seekTo).toBeNull();
  });
});

describe("isUserGesture", () => {
  it("treats events inside the programmatic window as our own", () => {
    expect(isUserGesture(1000, 950)).toBe(false); // 50ms after our mutation
  });

  it("treats events well after our last mutation as user gestures", () => {
    expect(isUserGesture(2000, 1000)).toBe(true); // 1s later
  });
});
