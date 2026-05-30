import { describe, expect, it } from "vitest";

import { translate, missingKeys, LANGS, LANG_NAMES } from "./i18n";

describe("translate", () => {
  it("returns the locale string when present", () => {
    expect(translate("no", "navExport")).toBe("Eksport");
    expect(translate("en", "navExport")).toBe("Export");
  });

  it("falls back to English for a key a locale is missing", () => {
    // No real key is missing now, so exercise the fallback path with a key
    // that exists in no catalog — translate() must return English (here: the
    // raw key, since en lacks it too) rather than throwing.
    const bogus = "totallyUnknownKey" as never;
    expect(translate("sv", bogus)).toBe(translate("en", bogus));
  });

  it("every locale carries the full catalog (no missing keys)", () => {
    for (const lang of LANGS) {
      expect(missingKeys(lang)).toEqual([]);
    }
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
