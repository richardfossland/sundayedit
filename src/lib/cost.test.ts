import { describe, expect, it } from "vitest";

import { formatCost } from "./cost";

describe("formatCost", () => {
  it("collapses sub-cent estimates to a single threshold label", () => {
    expect(formatCost(0)).toBe("<$0,01");
    expect(formatCost(0.004)).toBe("<$0,01");
    expect(formatCost(0.009)).toBe("<$0,01");
  });

  it("renders a cent and above with two decimals", () => {
    expect(formatCost(0.01)).toBe("$0.01");
    expect(formatCost(0.5)).toBe("$0.50");
    expect(formatCost(12.345)).toBe("$12.35");
  });
});
