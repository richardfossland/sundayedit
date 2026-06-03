/**
 * NLE timeline (Phase 1.3) — the spatial canvas where captions align to audio.
 *
 * A fixed-size viewport that renders the visible time window (pan = scrollMs,
 * zoom = pxPerMs) rather than one enormous scrolled element — so a 90-minute
 * project stays smooth. The waveform is drawn windowed into a canvas; captions
 * are a virtualized DOM track (only the visible window gets nodes). Drag a
 * caption to move it, drag its edges to retime; both commit through the pure
 * backend ops (clamped to neighbours).
 *
 * Transport is J/K/L shuttle (reverse/stop/forward, doubling on repeat) plus
 * Space; an internal playhead clock advances by the signed rate so the timeline
 * is usable on its own — a real <video> can attach to the same playheadMs/rate
 * contract later. Drags snap to neighbouring caption edges, the playhead and the
 * bounds (S toggles snapping).
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { appCacheDir } from "@tauri-apps/api/path";
import { ZoomIn, ZoomOut, Play, Pause, Magnet } from "lucide-react";

import type { Caption, Project, WaveformData } from "@/lib/bindings";
import { confidenceTier } from "@/lib/bindings";
import { ipc } from "@/lib/ipc";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";
import * as tl from "./geometry";
import { MediaPlayer } from "./MediaPlayer";

interface Props {
  project: Project;
  onProjectChange: (p: Project) => void;
  /** Asset URL for the source video, or undefined when none is attachable
   *  (browser/demo, no Tauri asset protocol). When set, a <video> is bound to
   *  the playhead clock instead of the static timecode placeholder. */
  videoSrc?: string;
}

const RULER_H = 22;
const WAVE_H = 72;
const CAPTION_H = 56;

type Drag = {
  kind: "move" | "resize-start" | "resize-end";
  id: string;
  startClientX: number;
  origStart: number;
  origEnd: number;
  deltaMs: number;
};

/** Worst (highest) confidence tier across a caption's words → box tint. */
function worstTier(c: Caption): number {
  let worst = 1;
  for (const w of c.words) {
    const t = confidenceTier(w);
    if (t > worst) worst = t;
  }
  return worst;
}

const TIER_BORDER: Record<number, string> = {
  1: "var(--color-success)",
  2: "var(--color-warning)",
  3: "var(--color-danger)",
  4: "var(--color-danger)",
};

export function Timeline({ project, onProjectChange, videoSrc }: Props) {
  const t = useT();
  const durationMs = Math.max(1, project.video_duration_ms);
  const fps = project.video_fps > 0 ? project.video_fps : 30;

  const captions = useMemo(
    () => [...project.captions].sort((a, b) => a.start_ms - b.start_ms),
    [project.captions],
  );

  const [view, setView] = useState<tl.TimelineView>({
    pxPerMs: 0.05,
    scrollMs: 0,
    widthPx: 800,
  });
  const [playheadMs, setPlayheadMs] = useState(0);
  // Signed playback rate (J/K/L shuttle): <0 reverse, 0 stopped, 1 realtime.
  const [rate, setRate] = useState(0);
  const playing = rate !== 0;
  const [snapEnabled, setSnapEnabled] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [waveform, setWaveform] = useState<WaveformData | null>(null);
  // A transient warning when the user scrubs the native video control while the
  // timeline is driving playback (the two clocks would fight).
  const [scrubWarning, setScrubWarning] = useState(false);

  const viewportRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  // Keep the visible window within [0, duration].
  const clampScroll = useCallback(
    (scrollMs: number, pxPerMs: number, widthPx: number) => {
      const span = widthPx / pxPerMs;
      return Math.max(0, Math.min(scrollMs, Math.max(0, durationMs - span)));
    },
    [durationMs],
  );

  // Measure the viewport width (drives the windowed render).
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth;
      setView((v) => ({
        ...v,
        widthPx: w,
        scrollMs: clampScroll(v.scrollMs, v.pxPerMs, w),
      }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [clampScroll]);

  // Fetch the real waveform once (no-op outside Tauri / without ffmpeg).
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cacheDir = await appCacheDir();
        const data = await ipc.project.waveform(project.video_path, cacheDir);
        if (!cancelled) setWaveform(data);
      } catch {
        // Browser/demo or no audio yet — render without a waveform track.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [project.video_path]);

  // Internal playback clock — advances the playhead by the shuttle `rate`
  // (direction + speed) in real time. Stops cleanly at either bound.
  useEffect(() => {
    if (rate === 0) return;
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = now - last;
      last = now;
      setPlayheadMs((p) => {
        const next = p + dt * rate;
        if (next <= 0) {
          setRate(0);
          return 0;
        }
        if (next >= durationMs) {
          setRate(0);
          return durationMs;
        }
        return next;
      });
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [rate, durationMs]);

  // Auto-dismiss the scrub-conflict warning a few seconds after it appears.
  useEffect(() => {
    if (!scrubWarning) return;
    const id = setTimeout(() => setScrubWarning(false), 4000);
    return () => clearTimeout(id);
  }, [scrubWarning]);

  // Draw the ruler-aligned waveform window into the canvas.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const width = view.widthPx;
    canvas.width = width * dpr;
    canvas.height = WAVE_H * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, WAVE_H);
    ctx.fillStyle = "rgba(255,255,255,0.02)";
    ctx.fillRect(0, 0, width, WAVE_H);

    if (!waveform || waveform.levels.length === 0) return;
    // Pick the pyramid level matching the full-duration content width.
    const targetBuckets = durationMs * view.pxPerMs;
    let level = waveform.levels[waveform.levels.length - 1];
    for (const lv of waveform.levels) {
      if (lv.length >= targetBuckets) {
        level = lv;
        break;
      }
    }
    if (!level || level.length === 0) return;

    const mid = WAVE_H / 2;
    const amp = WAVE_H / 2 - 2;
    ctx.strokeStyle = "rgba(79,209,197,0.85)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 0; x < width; x++) {
      const ms = tl.xToMs(x, view);
      if (ms < 0 || ms > durationMs) continue;
      const frac = ms / durationMs;
      const peak =
        level[Math.min(level.length - 1, Math.floor(frac * level.length))];
      if (!peak) continue;
      ctx.moveTo(x + 0.5, mid - peak.max * amp);
      ctx.lineTo(x + 0.5, mid - peak.min * amp);
    }
    ctx.stroke();
  }, [waveform, view, durationMs]);

  // ── interactions ───────────────────────────────────────────────────────────
  function seekToX(clientX: number) {
    const el = viewportRef.current;
    if (!el) return;
    const x = clientX - el.getBoundingClientRect().left;
    const ms = tl.snapToFrame(
      Math.max(0, Math.min(durationMs, tl.xToMs(x, view))),
      fps,
    );
    setPlayheadMs(ms);
  }

  function onWheel(e: React.WheelEvent) {
    const el = viewportRef.current;
    if (!el) return;
    if (e.ctrlKey || e.metaKey) {
      const anchorX = e.clientX - el.getBoundingClientRect().left;
      setView((v) => {
        const z = tl.zoomAround(v, e.deltaY < 0 ? 1.15 : 1 / 1.15, anchorX);
        return {
          ...z,
          scrollMs: clampScroll(z.scrollMs, z.pxPerMs, v.widthPx),
        };
      });
    } else {
      setView((v) => ({
        ...v,
        scrollMs: clampScroll(
          v.scrollMs + (e.deltaX || e.deltaY) / v.pxPerMs,
          v.pxPerMs,
          v.widthPx,
        ),
      }));
    }
  }

  function zoomButton(factor: number) {
    setView((v) => {
      const z = tl.zoomAround(v, factor, v.widthPx / 2);
      return { ...z, scrollMs: clampScroll(z.scrollMs, z.pxPerMs, v.widthPx) };
    });
  }

  // Caption drag (move / resize) — preview locally, commit on release.
  function onCaptionPointerDown(
    e: React.PointerEvent,
    c: Caption,
    kind: Drag["kind"],
  ) {
    e.stopPropagation();
    (e.target as Element).setPointerCapture?.(e.pointerId);
    setSelectedId(c.id);
    setDrag({
      kind,
      id: c.id,
      startClientX: e.clientX,
      origStart: c.start_ms,
      origEnd: c.end_ms,
      deltaMs: 0,
    });
  }

  function onPointerMove(e: React.PointerEvent) {
    if (!drag) return;
    const rawDelta = (e.clientX - drag.startClientX) / view.pxPerMs;
    // The edge the cursor is dragging: the start for move/resize-start, the
    // end for resize-end.
    const base = drag.kind === "resize-end" ? drag.origEnd : drag.origStart;
    let edge = base + rawDelta;
    if (snapEnabled) {
      // Snap to nearby caption boundaries, the playhead and the bounds.
      const [vs, ve] = tl.visibleRange(view);
      const targets = [0, durationMs, playheadMs];
      for (const { item } of tl.visibleCaptions(captions, vs, ve)) {
        if (item.id === drag.id) continue;
        targets.push(item.start_ms, item.end_ms);
      }
      edge = tl.snap(edge, targets, view.pxPerMs);
    }
    const deltaMs = tl.snapToFrame(edge, fps) - base;
    setDrag({ ...drag, deltaMs });
  }

  async function onPointerUp() {
    if (!drag) return;
    const d = drag;
    setDrag(null);
    if (d.deltaMs === 0) return;
    try {
      const next =
        d.kind === "move"
          ? await ipc.ops.moveCaption(project, d.id, d.deltaMs)
          : await ipc.ops.resizeCaption(
              project,
              d.id,
              d.kind === "resize-start" ? d.origStart + d.deltaMs : d.origStart,
              d.kind === "resize-end" ? d.origEnd + d.deltaMs : d.origEnd,
            );
      onProjectChange(next);
    } catch {
      // Clamped/invalid drag — leave the project untouched.
    }
  }

  function selectAndSeek(c: Caption) {
    setSelectedId(c.id);
    setPlayheadMs(c.start_ms);
  }

  function step(dir: -1 | 1, count: number) {
    const idx = captions.findIndex((c) => c.id === selectedId);
    if (idx === -1) {
      // Nothing selected → frame-step the playhead.
      setPlayheadMs((p) =>
        tl.snapToFrame(
          Math.max(0, Math.min(durationMs, p + ((dir * 1000) / fps) * count)),
          fps,
        ),
      );
      return;
    }
    const next =
      captions[Math.max(0, Math.min(captions.length - 1, idx + dir * count))];
    if (next) selectAndSeek(next);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const lower = e.key.toLowerCase();
    if (lower === "j" || lower === "k" || lower === "l") {
      e.preventDefault();
      setRate((r) => tl.shuttleRate(r, lower));
      return;
    }
    if (lower === "s") {
      e.preventDefault();
      setSnapEnabled((s) => !s);
      return;
    }
    switch (e.key) {
      case " ":
        e.preventDefault();
        setRate((r) => (r !== 0 ? 0 : 1));
        break;
      case "ArrowLeft":
        e.preventDefault();
        step(-1, e.metaKey || e.shiftKey ? 5 : 1);
        break;
      case "ArrowRight":
        e.preventDefault();
        step(1, e.metaKey || e.shiftKey ? 5 : 1);
        break;
      case "Home":
        e.preventDefault();
        if (captions[0]) selectAndSeek(captions[0]);
        break;
      case "End":
        e.preventDefault();
        if (captions.length) selectAndSeek(captions[captions.length - 1]);
        break;
    }
  }

  // ── render ───────────────────────────────────────────────────────────────
  const [visStart, visEnd] = tl.visibleRange(view);
  const visible = tl.visibleCaptions(captions, visStart, visEnd);
  const ticks = tl.rulerTicks(view, 80);
  const playheadX = tl.msToX(playheadMs, view);

  return (
    <div
      className="flex h-full flex-col bg-[var(--color-bg)] text-[var(--color-fg)] outline-none"
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      {/* Preview area — a real <video> bound to the playhead clock when one is
          attachable (videoSrc), else the timecode placeholder. */}
      <div className="relative flex flex-[3] items-center justify-center border-b border-[var(--color-border)] bg-black/40">
        {videoSrc ? (
          <MediaPlayer
            src={videoSrc}
            playheadMs={playheadMs}
            rate={rate}
            durationMs={durationMs}
            fps={fps}
            onConflict={() => setScrubWarning(true)}
          />
        ) : (
          <div className="text-center">
            <div className="font-mono text-[var(--text-ui-2xl)] tabular-nums">
              {tl.formatTimecode(playheadMs, fps)}
            </div>
            <div className="mt-1 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
              {project.name} · {tl.formatTimecode(durationMs, fps)}{" "}
              {t("timelineTotalSuffix")}
            </div>
          </div>
        )}
        {scrubWarning && (
          <div
            role="alert"
            className="absolute inset-x-0 bottom-0 bg-[var(--color-warning)]/90 px-3 py-1 text-center text-[var(--text-ui-xs)] text-[var(--color-neutral-950)]"
          >
            {t("mediaPlayerScrubWarning")}
          </div>
        )}
      </div>

      {/* Toolbar */}
      <div className="flex items-center gap-2 border-b border-[var(--color-border)] px-3 py-1.5">
        <button
          type="button"
          onClick={() => setRate((r) => (r !== 0 ? 0 : 1))}
          className="grid h-7 w-7 place-items-center rounded-md hover:bg-[var(--color-bg-surface)]"
          aria-label={playing ? t("timelinePause") : t("timelinePlay")}
        >
          {playing ? <Pause size={15} /> : <Play size={15} />}
        </button>
        <span className="font-mono text-[var(--text-ui-xs)] tabular-nums text-[var(--color-fg-muted)]">
          {tl.formatTimecode(playheadMs, fps)}
        </span>
        {rate !== 0 && rate !== 1 && (
          <span className="font-mono text-[var(--text-ui-xs)] tabular-nums text-[var(--color-accent-400)]">
            {rate < 0 ? `◂ ${-rate}×` : `${rate}× ▸`}
          </span>
        )}
        <div className="flex-1" />
        <button
          type="button"
          onClick={() => setSnapEnabled((s) => !s)}
          aria-pressed={snapEnabled}
          className={cn(
            "grid h-7 w-7 place-items-center rounded-md hover:bg-[var(--color-bg-surface)]",
            snapEnabled
              ? "text-[var(--color-accent-400)]"
              : "text-[var(--color-fg-subtle)]",
          )}
          aria-label={t("timelineSnap")}
          title={t("timelineSnap")}
        >
          <Magnet size={15} />
        </button>
        <span className="text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
          {(view.pxPerMs * 1000).toFixed(1)} px/s
        </span>
        <button
          type="button"
          onClick={() => zoomButton(1 / 1.3)}
          className="grid h-7 w-7 place-items-center rounded-md hover:bg-[var(--color-bg-surface)]"
          aria-label={t("timelineZoomOut")}
        >
          <ZoomOut size={15} />
        </button>
        <button
          type="button"
          onClick={() => zoomButton(1.3)}
          className="grid h-7 w-7 place-items-center rounded-md hover:bg-[var(--color-bg-surface)]"
          aria-label={t("timelineZoomIn")}
        >
          <ZoomIn size={15} />
        </button>
      </div>

      {/* Timeline viewport */}
      <div
        ref={viewportRef}
        className="relative flex-[2] select-none overflow-hidden"
        onWheel={onWheel}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => drag && onPointerUp()}
      >
        {/* Ruler */}
        <div
          className="relative border-b border-[var(--color-border)] bg-[var(--color-bg-elevated)]"
          style={{ height: RULER_H }}
          onPointerDown={(e) => seekToX(e.clientX)}
        >
          {ticks.map((ms) => (
            <div
              key={ms}
              className="absolute top-0 flex h-full items-center"
              style={{ left: tl.msToX(ms, view) }}
            >
              <span className="border-l border-[var(--color-border)] pl-1 font-mono text-[9px] text-[var(--color-fg-subtle)]">
                {tl.formatTimecode(ms, fps)}
              </span>
            </div>
          ))}
        </div>

        {/* Waveform */}
        <canvas
          ref={canvasRef}
          className="block w-full cursor-text"
          style={{ height: WAVE_H }}
          onPointerDown={(e) => seekToX(e.clientX)}
        />

        {/* Caption track */}
        <div
          className="relative border-t border-[var(--color-border)]"
          style={{ height: CAPTION_H }}
          onPointerDown={(e) => {
            seekToX(e.clientX);
            setSelectedId(null);
          }}
        >
          {visible.map(({ item: c }) => {
            const dragging = drag?.id === c.id ? drag : null;
            const start =
              c.start_ms +
              (dragging && dragging.kind !== "resize-end"
                ? dragging.deltaMs
                : 0);
            const end =
              c.end_ms +
              (dragging && dragging.kind !== "resize-start"
                ? dragging.deltaMs
                : 0);
            const left = tl.msToX(start, view);
            const width = Math.max(2, (end - start) * view.pxPerMs);
            const selected = selectedId === c.id;
            const tier = worstTier(c);
            return (
              <div
                key={c.id}
                className={cn(
                  "absolute top-1 bottom-1 overflow-hidden rounded border-l-2 bg-[var(--color-bg-surface)] text-[var(--text-ui-xs)]",
                  selected
                    ? "ring-2 ring-[var(--color-accent-500)]"
                    : "border-[var(--color-border)]",
                )}
                style={{ left, width, borderLeftColor: TIER_BORDER[tier] }}
                onPointerDown={(e) => onCaptionPointerDown(e, c, "move")}
                onClick={() => selectAndSeek(c)}
                title={c.words.map((w) => w.text).join(" ")}
              >
                {/* resize handles */}
                <span
                  className="absolute inset-y-0 left-0 w-1.5 cursor-ew-resize hover:bg-[var(--color-accent-500)]/40"
                  onPointerDown={(e) =>
                    onCaptionPointerDown(e, c, "resize-start")
                  }
                />
                <span
                  className="absolute inset-y-0 right-0 w-1.5 cursor-ew-resize hover:bg-[var(--color-accent-500)]/40"
                  onPointerDown={(e) =>
                    onCaptionPointerDown(e, c, "resize-end")
                  }
                />
                <div className="truncate px-2 py-1 text-[var(--color-fg-muted)]">
                  {c.words.map((w) => w.text).join(" ") || "—"}
                </div>
              </div>
            );
          })}
        </div>

        {/* Playhead across all tracks */}
        {playheadX >= 0 && playheadX <= view.widthPx && (
          <div
            className="pointer-events-none absolute top-0 bottom-0 w-px bg-white/90"
            style={{ left: playheadX }}
          />
        )}
      </div>

      <div className="border-t border-[var(--color-border)] px-3 py-1 text-[10px] text-[var(--color-fg-subtle)]">
        {t("timelineHelp")}
      </div>
    </div>
  );
}
