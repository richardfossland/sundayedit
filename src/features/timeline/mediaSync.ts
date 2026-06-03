/**
 * Pure media-clock sync — the math that keeps a <video> element following the
 * timeline's playhead clock (Phase 1.3 follow-up).
 *
 * The timeline owns the truth: a signed `rate` (0 = paused, <0 reverse, 1 =
 * realtime, doubling on shuttle) and a `playheadMs`. A <video> element can only
 * play forward at a non-negative rate and exposes `currentTime` in seconds, so
 * MediaPlayer reconciles the two contracts every animation frame. Keeping the
 * decision logic here — free of React/DOM/rAF — lets us unit-test the tricky
 * parts (when to seek, when to (un)pause, clamping to [0, duration], frame
 * precision) offline.
 */

/** What the timeline wants the video to be doing this frame. */
export interface VideoIntent {
  /** Target time the video should be at, in seconds (clamped to [0, dur]). */
  timeSec: number;
  /** Whether the video element should be playing. */
  shouldPlay: boolean;
  /** Native playbackRate for the element (always > 0; HTML can't reverse). */
  playbackRate: number;
}

/** What the video element is currently doing — the bits we reconcile against. */
export interface VideoActual {
  currentTimeSec: number;
  paused: boolean;
  durationSec: number;
}

// Browsers can't render <video> in reverse and stutter below ~0.0625×, so we
// only ever drive the element forward at realtime; reverse/shuttle scrubs the
// frame via seeks instead (handled by the seek threshold below).
const MAX_NATIVE_RATE = 1;
// Re-seek the element only when it has drifted more than ~one 30fps frame from
// the playhead — avoids fighting the element's own playback clock every frame.
const SEEK_EPSILON_SEC = 1 / 30;

/** Round a time (seconds) to the nearest frame boundary for the given fps. */
export function snapToFrameSec(sec: number, fps: number): number {
  if (fps <= 0) return sec;
  return Math.round(sec * fps) / fps;
}

/**
 * Translate the timeline's (playheadMs, rate) into what the <video> should do,
 * given the video's true duration. The playhead is the source of truth for
 * position; `rate === 1` is the only state the element plays natively (any
 * shuttle/reverse speed scrubs by seeking the playhead instead, so the element
 * stays paused and just follows along).
 */
export function intentFor(
  playheadMs: number,
  rate: number,
  durationSec: number,
  fps: number,
): VideoIntent {
  const dur = Math.max(0, durationSec);
  const timeSec = snapToFrameSec(
    Math.max(0, Math.min(dur, playheadMs / 1000)),
    fps,
  );
  // Only realtime forward playback lets the element drive itself; reverse and
  // fast/slow shuttle are realised as per-frame seeks (element stays paused).
  const shouldPlay = rate === 1 && timeSec < dur;
  return { timeSec, shouldPlay, playbackRate: MAX_NATIVE_RATE };
}

/** A reconciliation step: the concrete element mutations to apply this frame. */
export interface SyncStep {
  /** New currentTime to set, or null to leave the element's clock alone. */
  seekTo: number | null;
  /** Call play(), pause(), or neither. */
  transport: "play" | "pause" | null;
}

/**
 * Compare what the timeline wants (`intent`) with what the element is doing
 * (`actual`) and return only the mutations needed — so we don't reset
 * currentTime (which restarts decode) or call play()/pause() redundantly.
 *
 * While the element is playing realtime we let its own clock run and don't
 * chase it with seeks (that's what `SEEK_EPSILON_SEC` guards); while paused
 * (shuttle/reverse/stopped) we keep currentTime pinned to the playhead so the
 * frame under the playhead is always shown.
 */
export function reconcile(intent: VideoIntent, actual: VideoActual): SyncStep {
  const wantPlaying = intent.shouldPlay;
  let transport: SyncStep["transport"] = null;
  if (wantPlaying && actual.paused) transport = "play";
  else if (!wantPlaying && !actual.paused) transport = "pause";

  const drift = Math.abs(actual.currentTimeSec - intent.timeSec);
  // When playing, trust the element's clock unless it has drifted noticeably
  // (e.g. after a programmatic seek elsewhere). When paused, always pin it.
  const needsSeek = wantPlaying ? drift > SEEK_EPSILON_SEC : drift > 1e-3;
  const seekTo = needsSeek ? intent.timeSec : null;

  return { seekTo, transport };
}

/**
 * Did a `seeking`/`play`/`pause` event come from the user rather than our own
 * programmatic reconcile? We tag programmatic mutations with a short-lived
 * timestamp; an event arriving outside that window during playback is a manual
 * gesture (the user scrubbed/paused the native control) that conflicts with the
 * timeline driving the clock — the caller surfaces a warning.
 */
export function isUserGesture(
  eventAtMs: number,
  lastProgrammaticAtMs: number,
  windowMs = 150,
): boolean {
  return eventAtMs - lastProgrammaticAtMs > windowMs;
}
