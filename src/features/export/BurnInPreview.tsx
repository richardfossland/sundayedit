/**
 * Burn-in preview for the export panel.
 *
 * Before committing to a 10+ minute MP4 render, this renders the project's
 * caption styling *visually* — same `styleToCss` path the StyleEditor uses,
 * so it mirrors the eventual libass burn-in — framed to the selected
 * platform preset's aspect ratio (16:9 / 9:16 / 1:1). The user can sanity-
 * check fonts, colours, outline and position against the target frame
 * without waiting for ffmpeg.
 *
 * `previewFrame` is a pure helper (aspect-ratio + sample text) so it can be
 * unit-tested without a DOM.
 */

import { useEffect, useRef, useState } from "react";

import type { ExportPreset, Project } from "@/lib/bindings";
import { styleToCss } from "@/lib/styleToCss";
import { useT } from "@/lib/i18n";
import { previewAspectRatio, previewSampleText } from "./burnInPreview.helpers";

interface Props {
  project: Project;
  preset: ExportPreset;
}

export function BurnInPreview({ project, preset }: Props) {
  const t = useT();

  // Track the rendered frame height so font sizes scale proportionally —
  // identical strategy to StyleEditor's live preview.
  const frameRef = useRef<HTMLDivElement | null>(null);
  const [frameHeight, setFrameHeight] = useState(240);
  useEffect(() => {
    const el = frameRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setFrameHeight(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const css = styleToCss(project.default_style, frameHeight);
  const sample = previewSampleText(project, t("styleSampleText"));

  return (
    <div className="mt-4">
      <div
        ref={frameRef}
        data-testid="burnin-preview-frame"
        className="relative mx-auto w-full max-w-md overflow-hidden rounded-lg border border-[var(--color-border)]"
        style={{
          aspectRatio: previewAspectRatio(preset),
          // Stand-in "footage" gradient — same as the style editor, so the
          // user judges legibility against a realistic background.
          background:
            "linear-gradient(135deg, #2b3a55 0%, #3a4a3a 45%, #6b5b3a 100%)",
        }}
      >
        {/* TV-safe guide */}
        <div className="pointer-events-none absolute inset-[5%] rounded border border-dashed border-white/15" />
        <div style={css.container}>
          <span data-testid="burnin-preview-caption" style={css.text}>
            {sample}
          </span>
        </div>
      </div>
      <p className="mt-2 text-[var(--text-ui-xs)] text-[var(--color-fg-subtle)]">
        {t("exportPreviewHint")}
      </p>
    </div>
  );
}
