/**
 * Pure helpers for the burn-in preview (kept out of the component file so
 * the React-refresh boundary stays clean and they're unit-testable).
 */

import type { ExportPreset, Project } from "@/lib/bindings";

/** First non-empty caption's text, or a stand-in if the project has none. */
export function previewSampleText(project: Project, fallback: string): string {
  for (const c of project.captions) {
    const text = c.words
      .map((w) => w.text)
      .join(" ")
      .trim();
    if (text.length > 0) return text;
  }
  return fallback;
}

/** CSS aspect-ratio string ("w / h") for the preset frame. */
export function previewAspectRatio(preset: ExportPreset): string {
  return `${preset.width} / ${preset.height}`;
}
