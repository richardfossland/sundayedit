import { describe, it, expect } from "vitest";
import { styleToCss } from "./styleToCss";
import type { Style } from "./bindings";

function makeStyle(overrides: Partial<Style> = {}): Style {
  return {
    id: "s1",
    name: "Test",
    font_family: "Inter",
    font_size_px: 48,
    font_weight: 700,
    italic: false,
    color_fg: "#ffffff",
    outline_color: "#000000",
    outline_width_px: 3,
    shadow_color: "#000000",
    shadow_offset_x: 0,
    shadow_offset_y: 0,
    shadow_blur: 0,
    background_color: null,
    background_padding_px: 8,
    background_radius_px: 4,
    align_h: "center",
    align_v: "bottom",
    anchor: "bc",
    max_width_pct: 80,
    line_spacing: 1.2,
    letter_spacing: 0,
    animation: null,
    ...overrides,
  };
}

describe("styleToCss", () => {
  it("scales font size proportionally to the preview frame height", () => {
    // 48px @ 1080 reference, rendered into a 540px frame → half size.
    const { text } = styleToCss(makeStyle({ font_size_px: 48 }), 540);
    expect(text.fontSize).toBe("24px");
  });

  it("never renders font smaller than the 8px floor", () => {
    const { text } = styleToCss(makeStyle({ font_size_px: 10 }), 100);
    expect(text.fontSize).toBe("8px");
  });

  it("maps a bottom-center anchor to flex end/center", () => {
    const { container } = styleToCss(makeStyle({ anchor: "bc" }), 1080);
    expect(container.justifyContent).toBe("center");
    expect(container.alignItems).toBe("flex-end");
  });

  it("maps a top-left anchor to flex start/start", () => {
    const { container } = styleToCss(makeStyle({ anchor: "tl" }), 1080);
    expect(container.justifyContent).toBe("flex-start");
    expect(container.alignItems).toBe("flex-start");
  });

  it("builds an 8-direction outline text-shadow when outline width > 0", () => {
    const { text } = styleToCss(makeStyle({ outline_width_px: 4 }), 1080);
    // 8 offset layers separated by commas.
    expect(String(text.textShadow).split(",").length).toBe(8);
  });

  it("omits the outline when width is 0", () => {
    const { text } = styleToCss(makeStyle({ outline_width_px: 0 }), 1080);
    expect(text.textShadow).toBeUndefined();
  });

  it("applies background styling only when a background color is set", () => {
    const none = styleToCss(makeStyle({ background_color: null }), 1080);
    expect(none.text.backgroundColor).toBeUndefined();
    const filled = styleToCss(makeStyle({ background_color: "#101010" }), 1080);
    expect(filled.text.backgroundColor).toBe("#101010");
  });
});
