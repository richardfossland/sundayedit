import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, cleanup, screen } from "@testing-library/react";

import { BurnInPreview } from "./BurnInPreview";
import { previewAspectRatio, previewSampleText } from "./burnInPreview.helpers";
import { SAMPLE_PROJECT } from "@/lib/sampleProject";
import { styleToCss } from "@/lib/styleToCss";
import type { ExportPreset, Project } from "@/lib/bindings";

// jsdom has no ResizeObserver; the component installs one to track its frame
// height. A no-op stub is enough — without an observed resize the default
// frameHeight stands, which is fine for asserting frame-independent styling.
beforeEach(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

function makePreset(overrides: Partial<ExportPreset> = {}): ExportPreset {
  return {
    id: "youtube_16x9",
    name: "YouTube",
    description: "Landscape",
    aspect: "landscape",
    width: 1920,
    height: 1080,
    max_duration_sec: null,
    codec: "h264",
    bitrate_kbps: 8000,
    also_srt_sidecar: false,
    ...overrides,
  };
}

describe("previewSampleText", () => {
  it("returns the first non-empty caption's text", () => {
    expect(previewSampleText(SAMPLE_PROJECT, "fallback")).toBe(
      "Velkommen til gudstjenesten denne søndagen morgen",
    );
  });

  it("falls back when no caption has words", () => {
    const empty: Project = { ...SAMPLE_PROJECT, captions: [] };
    expect(previewSampleText(empty, "fallback")).toBe("fallback");
  });
});

describe("previewAspectRatio", () => {
  it("derives a CSS aspect ratio from the preset dimensions", () => {
    expect(previewAspectRatio(makePreset({ width: 1080, height: 1920 }))).toBe(
      "1080 / 1920",
    );
  });
});

describe("BurnInPreview", () => {
  it("renders the caption text with the SAME CSS the burn-in mirror produces", () => {
    render(<BurnInPreview project={SAMPLE_PROJECT} preset={makePreset()} />);

    const caption = screen.getByTestId("burnin-preview-caption") as HTMLElement;
    // The preview must reuse styleToCss (the preview side of "preview ==
    // output", which itself mirrors Rust's ASS/libass burn-in). Assert the
    // frame-independent style props are applied from styleToCss.
    const expected = styleToCss(SAMPLE_PROJECT.default_style, 240).text;

    // Normalise expected values through the CSSOM (jsdom rewrites colours to
    // rgb(...)) so we compare like-for-like with the rendered element.
    const probe = document.createElement("span");
    probe.style.color = String(expected.color);
    probe.style.fontFamily = String(expected.fontFamily);
    probe.style.textAlign = String(expected.textAlign);

    expect(caption.style.color).toBe(probe.style.color);
    expect(caption.style.fontFamily).toBe(probe.style.fontFamily);
    expect(caption.style.textAlign).toBe(probe.style.textAlign);
    expect(caption.textContent).toBe(
      "Velkommen til gudstjenesten denne søndagen morgen",
    );
  });

  it("frames the preview to the preset's aspect ratio", () => {
    render(
      <BurnInPreview
        project={SAMPLE_PROJECT}
        preset={makePreset({ width: 1080, height: 1920 })}
      />,
    );
    const frame = screen.getByTestId("burnin-preview-frame") as HTMLElement;
    expect(frame.style.aspectRatio).toBe("1080 / 1920");
  });
});
