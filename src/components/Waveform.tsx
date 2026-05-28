/**
 * Waveform display — Canvas rendering of multi-zoom peak data (Phase 1.2).
 *
 * Picks the peak level closest to the current pixel-per-second zoom, then
 * draws one vertical line per pixel column from `min` to `max`. Canvas
 * (not SVG) because a 90-minute file has far too many points for the DOM.
 *
 * Click to seek (reports the seeked time in ms via onSeek).
 */

import { useEffect, useRef } from "react";
import type { WaveformData, Peak } from "@/lib/bindings";

interface Props {
  data: WaveformData;
  /** Total media duration in ms — maps x position to time. */
  durationMs: number;
  /** Current playhead position in ms (draws the overlay line). */
  playheadMs?: number;
  height?: number;
  onSeek?: (ms: number) => void;
}

export function Waveform({ data, durationMs, playheadMs, height = 96, onSeek }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const dpr = window.devicePixelRatio || 1;
    const width = container.clientWidth;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, width, height);

    // Pick the level whose bucket count is closest to (but not less than)
    // the available pixel columns — we want at least one peak per column.
    const level = pickLevel(data.levels, width);
    if (!level || level.length === 0) return;

    const mid = height / 2;
    const amp = height / 2 - 2;

    // Background
    ctx.fillStyle = "rgba(255,255,255,0.02)";
    ctx.fillRect(0, 0, width, height);

    // Centre line
    ctx.strokeStyle = "rgba(255,255,255,0.08)";
    ctx.beginPath();
    ctx.moveTo(0, mid);
    ctx.lineTo(width, mid);
    ctx.stroke();

    // Peaks
    ctx.strokeStyle = "rgba(79,209,197,0.85)"; // teal accent
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let x = 0; x < width; x++) {
      const idx = Math.floor((x / width) * level.length);
      const peak = level[Math.min(idx, level.length - 1)];
      const yMin = mid - peak.max * amp;
      const yMax = mid - peak.min * amp;
      ctx.moveTo(x + 0.5, yMin);
      ctx.lineTo(x + 0.5, yMax);
    }
    ctx.stroke();

    // Playhead
    if (playheadMs !== undefined && durationMs > 0) {
      const x = (playheadMs / durationMs) * width;
      ctx.strokeStyle = "rgba(255,255,255,0.9)";
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }
  }, [data, durationMs, playheadMs, height]);

  function handleClick(e: React.MouseEvent<HTMLCanvasElement>) {
    if (!onSeek || durationMs <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const ms = Math.round((x / rect.width) * durationMs);
    onSeek(Math.max(0, Math.min(durationMs, ms)));
  }

  return (
    <div ref={containerRef} className="w-full">
      <canvas
        ref={canvasRef}
        onClick={handleClick}
        className="block w-full cursor-pointer rounded-md"
        style={{ height }}
      />
    </div>
  );
}

/** Choose the finest level that still has >= `targetColumns` buckets, or
 *  the finest available if none reach it. */
function pickLevel(levels: Peak[][], targetColumns: number): Peak[] | null {
  if (levels.length === 0) return null;
  for (const level of levels) {
    if (level.length >= targetColumns) return level;
  }
  return levels[levels.length - 1];
}
