import { describe, it, expect } from "vitest";
import { confidenceTier } from "./bindings";

const w = (confidence: number, edited = false, locked = false) => ({
  confidence,
  edited,
  locked,
});

describe("confidenceTier", () => {
  it("maps confidence to the four tiers at the documented boundaries", () => {
    expect(confidenceTier(w(100))).toBe(1);
    expect(confidenceTier(w(85))).toBe(1); // tier-1 floor
    expect(confidenceTier(w(84))).toBe(2);
    expect(confidenceTier(w(70))).toBe(2); // tier-2 floor
    expect(confidenceTier(w(69))).toBe(3);
    expect(confidenceTier(w(50))).toBe(3); // tier-3 floor
    expect(confidenceTier(w(49))).toBe(4);
    expect(confidenceTier(w(0))).toBe(4);
  });

  it("treats locked words as tier 1 even at low confidence", () => {
    expect(confidenceTier(w(10, false, true))).toBe(1);
  });

  it("treats user-edited words as tier 1 even at low confidence", () => {
    expect(confidenceTier(w(10, true, false))).toBe(1);
  });
});
