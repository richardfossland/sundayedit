/**
 * Style → CSS for the live WYSIWYG preview (Phase 5.1).
 *
 * This is the preview side of "preview == output". The burn-in side is
 * Rust's `export::format_ass_style` (libass). Both read the SAME `Style`
 * fields, so a change in the editor updates the preview AND the eventual
 * burned-in render consistently.
 *
 * The preview renders a caption inside a 16:9 "video frame" box; sizes
 * here are expressed relative to that box so the proportions match what
 * libass produces at the real output resolution.
 */

import type { Style } from "./bindings";

export interface CaptionBoxStyle {
  /** Positioning of the caption block within the frame. */
  container: React.CSSProperties;
  /** The text styling itself. */
  text: React.CSSProperties;
}

/**
 * @param style       the Style to render
 * @param frameHeight the preview frame height in px — font size scales
 *                    relative to a 1080p reference so the preview matches
 *                    libass output proportionally.
 */
export function styleToCss(style: Style, frameHeight: number): CaptionBoxStyle {
  // Scale px sizes from a 1080-tall reference to the preview frame.
  const scale = frameHeight / 1080;
  const fontPx = Math.max(8, Math.round(style.font_size_px * scale));
  const outlinePx = style.outline_width_px * scale;

  // 9-grid anchor → flex alignment of the container.
  const { justify, align } = anchorToFlex(style.anchor);

  const container: React.CSSProperties = {
    position: "absolute",
    inset: 0,
    display: "flex",
    justifyContent: justify,
    alignItems: align,
    padding: `${Math.round(24 * scale)}px`,
    pointerEvents: "none",
  };

  // Outline via layered text-shadow (the standard CSS approximation of
  // libass's border). Combine the requested drop shadow too.
  const outline = outlinePx > 0 ? buildOutline(style.outline_color, outlinePx) : "";
  const drop =
    style.shadow_blur > 0 || style.shadow_offset_x || style.shadow_offset_y
      ? `${style.shadow_offset_x * scale}px ${style.shadow_offset_y * scale}px ${style.shadow_blur * scale}px ${style.shadow_color}`
      : "";
  const textShadow = [outline, drop].filter(Boolean).join(", ");

  const text: React.CSSProperties = {
    fontFamily: style.font_family,
    fontWeight: style.font_weight,
    fontStyle: style.italic ? "italic" : "normal",
    fontSize: `${fontPx}px`,
    color: style.color_fg,
    textAlign: style.align_h as React.CSSProperties["textAlign"],
    lineHeight: style.line_spacing,
    letterSpacing: `${style.letter_spacing}px`,
    maxWidth: `${style.max_width_pct}%`,
    textShadow: textShadow || undefined,
    ...(style.background_color
      ? {
          backgroundColor: style.background_color,
          padding: `${style.background_padding_px * scale}px ${style.background_padding_px * 1.5 * scale}px`,
          borderRadius: `${style.background_radius_px * scale}px`,
        }
      : {}),
  };

  return { container, text };
}

/** 8-direction outline approximation for CSS text. */
function buildOutline(color: string, width: number): string {
  const w = Math.max(0.5, width);
  const offsets: Array<[number, number]> = [
    [-w, -w], [0, -w], [w, -w],
    [-w, 0],            [w, 0],
    [-w, w],  [0, w],   [w, w],
  ];
  return offsets.map(([x, y]) => `${x}px ${y}px 0 ${color}`).join(", ");
}

function anchorToFlex(anchor: string): { justify: React.CSSProperties["justifyContent"]; align: React.CSSProperties["alignItems"] } {
  const horizontal = anchor[1]; // l | c | r
  const vertical = anchor[0];   // t | m | b
  const justify =
    horizontal === "l" ? "flex-start" : horizontal === "r" ? "flex-end" : "center";
  const align =
    vertical === "t" ? "flex-start" : vertical === "b" ? "flex-end" : "center";
  return { justify, align };
}
