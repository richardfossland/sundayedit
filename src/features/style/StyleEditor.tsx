/**
 * Visual style editor — Phase 5.2.
 *
 * Left: live preview of a sample caption over a stand-in "video frame"
 * at 16:9, rendered via styleToCss so it mirrors the eventual burn-in.
 * Right: preset gallery + a few core controls (font size, colour,
 * outline, position). Pick a preset to apply instantly.
 *
 * The preview uses the same Style object that the export/burn-in path
 * consumes, so what you see is what gets rendered.
 */

import { useMemo, useRef, useState, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";

import type { Style } from "@/lib/bindings";
import { ipc } from "@/lib/ipc";
import { useT } from "@/lib/i18n";
import { styleToCss } from "@/lib/styleToCss";
import { cn } from "@/lib/cn";

interface Props {
  style: Style;
  onChange: (style: Style) => void;
}

const ANCHORS: Array<{ key: string; label: string }> = [
  { key: "tl", label: "↖" },
  { key: "tc", label: "↑" },
  { key: "tr", label: "↗" },
  { key: "ml", label: "←" },
  { key: "mc", label: "•" },
  { key: "mr", label: "→" },
  { key: "bl", label: "↙" },
  { key: "bc", label: "↓" },
  { key: "br", label: "↘" },
];

export function StyleEditor({ style, onChange }: Props) {
  const t = useT();
  const presetsQuery = useQuery({
    queryKey: ["style-presets"],
    queryFn: () => ipc.style.listPresets(),
  });
  const presets = presetsQuery.data ?? [];

  const frameRef = useRef<HTMLDivElement | null>(null);
  const [frameHeight, setFrameHeight] = useState(360);
  useEffect(() => {
    const el = frameRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setFrameHeight(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const css = useMemo(
    () => styleToCss(style, frameHeight),
    [style, frameHeight],
  );

  function patch(p: Partial<Style>) {
    onChange({ ...style, ...p });
  }

  return (
    <div className="grid h-full grid-cols-[1fr_320px] overflow-hidden">
      {/* Preview */}
      <div className="flex flex-col items-center justify-center gap-3 overflow-auto p-8">
        <div
          ref={frameRef}
          className="relative aspect-video w-full max-w-3xl overflow-hidden rounded-lg border border-[var(--color-border)]"
          style={{
            // Stand-in "footage": a gradient so users can judge legibility.
            background:
              "linear-gradient(135deg, #2b3a55 0%, #3a4a3a 45%, #6b5b3a 100%)",
          }}
        >
          {/* Safe-area guide (TV-safe ~ 90%) */}
          <div className="pointer-events-none absolute inset-[5%] rounded border border-dashed border-white/15" />
          <div style={css.container}>
            <span style={css.text}>{t("styleSampleText")}</span>
          </div>
        </div>
        <p className="text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
          {t("stylePreviewHint")}
        </p>
      </div>

      {/* Controls */}
      <div className="flex flex-col gap-5 overflow-y-auto border-l border-[var(--color-border)] p-4">
        {/* Preset gallery */}
        <section>
          <h3 className="mb-2 text-[var(--text-ui-xs)] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
            {t("stylePresets")}
          </h3>
          <div className="grid grid-cols-2 gap-1.5">
            {presets.map((p) => (
              <button
                key={p.style.id}
                type="button"
                onClick={() => onChange(p.style)}
                className={cn(
                  "rounded-md border px-2.5 py-2 text-left transition-colors",
                  style.id === p.style.id
                    ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/8"
                    : "border-[var(--color-border)] hover:border-[var(--color-border-strong)]",
                )}
                title={p.description}
              >
                <div className="text-[var(--text-ui-sm)] font-medium">
                  {p.style.name}
                </div>
                <div className="text-[10px] text-[var(--color-fg-subtle)]">
                  {p.category}
                </div>
              </button>
            ))}
          </div>
        </section>

        {/* Font size */}
        <Control label={t("styleFontSize", { px: style.font_size_px })}>
          <input
            type="range"
            min={20}
            max={120}
            value={style.font_size_px}
            onChange={(e) => patch({ font_size_px: Number(e.target.value) })}
            className="w-full accent-[var(--color-accent-500)]"
          />
        </Control>

        {/* Colours */}
        <Control label={t("styleColors")}>
          <div className="flex items-center gap-4">
            <ColorField
              label={t("styleColorText")}
              value={style.color_fg}
              onChange={(v) => patch({ color_fg: v })}
            />
            <ColorField
              label={t("styleColorOutline")}
              value={style.outline_color}
              onChange={(v) => patch({ outline_color: v })}
            />
          </div>
        </Control>

        {/* Outline width */}
        <Control label={t("styleOutlineWidth", { px: style.outline_width_px })}>
          <input
            type="range"
            min={0}
            max={10}
            value={style.outline_width_px}
            onChange={(e) =>
              patch({ outline_width_px: Number(e.target.value) })
            }
            className="w-full accent-[var(--color-accent-500)]"
          />
        </Control>

        {/* Position 9-grid */}
        <Control label={t("stylePosition")}>
          <div className="grid w-fit grid-cols-3 gap-1">
            {ANCHORS.map((a) => (
              <button
                key={a.key}
                type="button"
                onClick={() =>
                  patch({
                    anchor: a.key,
                    align_v:
                      a.key[0] === "t"
                        ? "top"
                        : a.key[0] === "b"
                          ? "bottom"
                          : "middle",
                    align_h:
                      a.key[1] === "l"
                        ? "left"
                        : a.key[1] === "r"
                          ? "right"
                          : "center",
                  })
                }
                className={cn(
                  "grid h-8 w-8 place-items-center rounded text-[var(--text-ui-sm)]",
                  style.anchor === a.key
                    ? "bg-[var(--color-accent-500)] text-[var(--color-neutral-950)]"
                    : "bg-[var(--color-bg-surface)] text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
                )}
              >
                {a.label}
              </button>
            ))}
          </div>
        </Control>

        {/* Background toggle */}
        <Control label={t("styleBackgroundBox")}>
          <label className="flex items-center gap-2 text-[var(--text-ui-sm)]">
            <input
              type="checkbox"
              checked={style.background_color != null}
              onChange={(e) =>
                patch({
                  background_color: e.target.checked ? "#000000A0" : null,
                })
              }
              className="accent-[var(--color-accent-500)]"
            />
            {t("styleBackgroundToggle")}
          </label>
        </Control>
      </div>
    </div>
  );
}

function Control({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <h3 className="mb-2 text-[var(--text-ui-xs)] font-semibold uppercase tracking-wider text-[var(--color-fg-subtle)]">
        {label}
      </h3>
      {children}
    </section>
  );
}

function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  // Strip any 8-digit alpha for the native picker; keep full value in state.
  const base = value.length > 7 ? value.slice(0, 7) : value;
  return (
    <label className="flex items-center gap-2 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
      <input
        type="color"
        value={base}
        onChange={(e) => onChange(e.target.value)}
        className="h-7 w-7 cursor-pointer rounded border border-[var(--color-border)] bg-transparent"
      />
      {label}
    </label>
  );
}
