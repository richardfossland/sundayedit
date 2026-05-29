import { describe, expect, it } from "vitest";

import { translate, LANGS, LANG_NAMES } from "./i18n";

describe("translate", () => {
  it("returns the locale string when present", () => {
    expect(translate("no", "navExport")).toBe("Eksport");
    expect(translate("en", "navExport")).toBe("Export");
  });

  it("falls back to English for a key a partial locale hasn't translated", () => {
    // sv only ships nav/chrome; clipsTitle is English-only there.
    expect(translate("sv", "clipsTitle")).toBe(translate("en", "clipsTitle"));
  });

  it("interpolates {name}-style tokens", () => {
    expect(translate("en", "updateAvailable", { version: "4.2.0" })).toBe(
      "Version 4.2.0 is available.",
    );
    expect(translate("no", "cleanupMatches", { n: 3 })).toBe("3 treff");
  });

  it("has an autonym for every supported language", () => {
    for (const lang of LANGS) {
      expect(LANG_NAMES[lang]).toBeTruthy();
    }
  });
});
