import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, cleanup, act } from "@testing-library/react";

import { MediaPlayer } from "./MediaPlayer";

// jsdom implements <video> as an element but not playback: play()/pause() are
// stubbed to throw "Not implemented", and currentTime/duration/paused are not
// wired to a media clock. We mock the bits MediaPlayer reconciles against so we
// can observe the calls it makes (offline — no real video is loaded).
let mockState: { currentTime: number; paused: boolean; duration: number };
let playSpy: ReturnType<typeof vi.spyOn>;
let pauseSpy: ReturnType<typeof vi.spyOn>;

function installVideoMock() {
  mockState = { currentTime: 0, paused: true, duration: 60 };
  const proto = window.HTMLMediaElement.prototype;
  playSpy = vi.spyOn(proto, "play").mockImplementation(() => {
    mockState.paused = false;
    return Promise.resolve();
  });
  pauseSpy = vi.spyOn(proto, "pause").mockImplementation(() => {
    mockState.paused = true;
  });
  vi.spyOn(proto, "currentTime", "get").mockImplementation(
    () => mockState.currentTime,
  );
  vi.spyOn(proto, "currentTime", "set").mockImplementation((v: number) => {
    mockState.currentTime = v;
  });
  vi.spyOn(proto, "paused", "get").mockImplementation(() => mockState.paused);
  vi.spyOn(proto, "duration", "get").mockImplementation(
    () => mockState.duration,
  );
}

// Drive the requestAnimationFrame loop deterministically.
let rafCb: FrameRequestCallback | null = null;
function installRaf() {
  rafCb = null;
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
    rafCb = cb;
    return 1;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
}
function pumpFrame() {
  act(() => {
    rafCb?.(performance.now());
  });
}

beforeEach(() => {
  installVideoMock();
  installRaf();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("MediaPlayer", () => {
  it("seeks the element to the playhead while paused (rate 0)", () => {
    render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={2000}
        rate={0}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(mockState.currentTime).toBeCloseTo(2.0);
    expect(playSpy).not.toHaveBeenCalled();
  });

  it("plays the element when the timeline plays at realtime", () => {
    mockState.currentTime = 2.0;
    render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={2000}
        rate={1}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(playSpy).toHaveBeenCalled();
  });

  it("keeps playing to the timeline end when the element duration is shorter than the project metadata", () => {
    // Probe metadata gives durationMs=60_000, but the real container is 59.5s.
    // The timeline clock (authority) keeps advancing to 60s, so the element
    // must keep playing rather than pausing early at its own 59.5s end — else
    // the playhead desyncs from the frozen video near the clip end.
    mockState.duration = 59.5;
    mockState.currentTime = 59.4;
    mockState.paused = true;
    render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={59_600}
        rate={1}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(playSpy).toHaveBeenCalled();
    expect(pauseSpy).not.toHaveBeenCalled();
  });

  it("pauses the element for reverse/shuttle (scrubs by seeking instead)", () => {
    mockState.paused = false;
    render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={3000}
        rate={-2}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(pauseSpy).toHaveBeenCalled();
    expect(mockState.currentTime).toBeCloseTo(3.0);
  });

  it("follows a moving playhead across re-renders", () => {
    const { rerender } = render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={1000}
        rate={0}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(mockState.currentTime).toBeCloseTo(1.0);
    rerender(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={5000}
        rate={0}
        durationMs={60_000}
        fps={30}
      />,
    );
    pumpFrame();
    expect(mockState.currentTime).toBeCloseTo(5.0);
  });

  it("warns on a manual scrub gesture during playback", () => {
    const onConflict = vi.fn();
    const { container } = render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={1000}
        rate={1}
        durationMs={60_000}
        fps={30}
        onConflict={onConflict}
      />,
    );
    const video = container.querySelector("video")!;
    // No programmatic mutation just happened, and we're playing → user gesture.
    act(() => {
      video.dispatchEvent(new Event("seeking"));
    });
    expect(onConflict).toHaveBeenCalled();
  });

  it("does not warn on a seek while the timeline is stopped", () => {
    const onConflict = vi.fn();
    const { container } = render(
      <MediaPlayer
        src="asset://x.mp4"
        playheadMs={1000}
        rate={0}
        durationMs={60_000}
        fps={30}
        onConflict={onConflict}
      />,
    );
    const video = container.querySelector("video")!;
    act(() => {
      video.dispatchEvent(new Event("seeking"));
    });
    expect(onConflict).not.toHaveBeenCalled();
  });

  it("shows the unavailable overlay when the video errors", () => {
    const { container, queryByTestId } = render(
      <MediaPlayer
        src="asset://missing.mp4"
        playheadMs={0}
        rate={0}
        durationMs={60_000}
        fps={30}
      />,
    );
    expect(queryByTestId("media-unavailable")).toBeNull();
    const video = container.querySelector("video")!;
    act(() => {
      video.dispatchEvent(new Event("error"));
    });
    expect(queryByTestId("media-unavailable")).not.toBeNull();
  });
});
