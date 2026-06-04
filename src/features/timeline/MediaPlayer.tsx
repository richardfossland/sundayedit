/**
 * MediaPlayer (Phase 1.3 follow-up) — binds a real <video> to the timeline's
 * playhead clock so the user can watch while transcribing/editing captions.
 *
 * The timeline stays the source of truth: it owns `playheadMs` and the signed
 * shuttle `rate`. This component is a thin reconciler — every animation frame
 * it asks `mediaSync` what the element should be doing and applies only the
 * needed mutations (seek / play / pause). Reverse and fast/slow shuttle can't
 * be done by a browser <video>, so those states scrub the element frame-by-
 * frame via seeks (it stays paused and just follows the playhead).
 *
 * Native transport controls are hidden, but a user can still scrub via the OS
 * media keys / picture-in-picture; if that happens during playback we raise a
 * warning rather than silently fighting them.
 */

import { useEffect, useRef, useState } from "react";

import { useT } from "@/lib/i18n";
import { intentFor, reconcile, isUserGesture } from "./mediaSync";

interface Props {
  /** Asset URL for the video (already run through convertFileSrc by App). */
  src: string;
  /** Timeline playhead, ms — the position the element should show. */
  playheadMs: number;
  /** Signed shuttle rate: 0 stopped, <0 reverse, 1 realtime, doubling. */
  rate: number;
  /** Project duration, ms — the clamp bound when metadata isn't loaded yet. */
  durationMs: number;
  /** Frames per second, for snapping seeks to frame boundaries. */
  fps: number;
  /** Raised when the user manually scrubs/pauses while the timeline plays. */
  onConflict?: () => void;
}

export function MediaPlayer({
  src,
  playheadMs,
  rate,
  durationMs,
  fps,
  onConflict,
}: Props) {
  const t = useT();
  const videoRef = useRef<HTMLVideoElement | null>(null);
  // Timestamp of our most recent programmatic mutation — lets event handlers
  // tell our own seeks/play/pause apart from the user's.
  const lastProgrammaticAtMs = useRef(0);
  // The video file may not be on disk (dev/demo) or may be an unsupported
  // codec — fall back to the timeline-only experience and say why.
  const [unavailable, setUnavailable] = useState(false);

  // Reconcile the element to the playhead every frame. We read the latest props
  // off refs so the rAF loop doesn't restart on every playhead tick.
  const stateRef = useRef({ playheadMs, rate, durationMs, fps });
  stateRef.current = { playheadMs, rate, durationMs, fps };

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const video = videoRef.current;
      if (video) {
        const s = stateRef.current;
        // The timeline's durationMs is the authority for when playback ends
        // (same domain we export against). The element's own duration may
        // disagree slightly (probe metadata vs container length); pass it only
        // to clamp the seek target so we never seek past real footage. This
        // keeps the preview and the timeline clock stopping at the same end.
        const timelineDurSec = s.durationMs / 1000;
        const elementDurSec = Number.isFinite(video.duration)
          ? video.duration
          : timelineDurSec;
        const intent = intentFor(
          s.playheadMs,
          s.rate,
          timelineDurSec,
          s.fps,
          elementDurSec,
        );
        const step = reconcile(intent, {
          currentTimeSec: video.currentTime,
          paused: video.paused,
          durationSec: elementDurSec,
        });
        if (step.seekTo !== null || step.transport) {
          lastProgrammaticAtMs.current = performance.now();
        }
        if (step.seekTo !== null) video.currentTime = step.seekTo;
        if (step.transport === "play") void video.play().catch(() => {});
        else if (step.transport === "pause") video.pause();
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  // A play/pause/seeking event outside our programmatic window, while the
  // timeline is playing, is a conflicting manual gesture — warn the user.
  function onMaybeUserGesture() {
    if (stateRef.current.rate === 0) return;
    if (isUserGesture(performance.now(), lastProgrammaticAtMs.current)) {
      onConflict?.();
    }
  }

  return (
    <div className="relative grid h-full place-items-center overflow-hidden">
      {/* Captions are the timeline's job; this is a driven preview surface,
          not delivered media, so it carries no <track>. */}
      <video
        ref={videoRef}
        src={src}
        className="max-h-full max-w-full"
        // Timeline owns transport; the element is driven, not user-controlled.
        controls={false}
        muted={false}
        playsInline
        preload="auto"
        onError={() => setUnavailable(true)}
        onPlay={onMaybeUserGesture}
        onPause={onMaybeUserGesture}
        onSeeking={onMaybeUserGesture}
      />
      {unavailable && (
        <div
          className="pointer-events-none absolute inset-0 grid place-items-center bg-black/60 px-4 text-center text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]"
          role="status"
        >
          <span data-testid="media-unavailable">{t("mediaPlayerMissing")}</span>
        </div>
      )}
    </div>
  );
}
