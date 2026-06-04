import { describe, expect, it } from "vitest";

import {
  EMPTY_SELECTION,
  clear,
  count,
  isContiguous,
  isSelected,
  orderedSelection,
  selectAll,
  selectRange,
  toggle,
} from "./selection";

const IDS = ["c1", "c2", "c3", "c4", "c5"];

describe("selection — toggle", () => {
  it("adds an absent id and removes a present one", () => {
    const s1 = toggle(EMPTY_SELECTION, "c2");
    expect(isSelected(s1, "c2")).toBe(true);
    const s2 = toggle(s1, "c2");
    expect(isSelected(s2, "c2")).toBe(false);
  });

  it("does not mutate the input selection", () => {
    const base = toggle(EMPTY_SELECTION, "c1");
    const next = toggle(base, "c2");
    expect(count(base)).toBe(1);
    expect(count(next)).toBe(2);
  });
});

describe("selection — clear / selectAll / count", () => {
  it("clear returns an empty selection", () => {
    const filled = selectAll(IDS);
    expect(count(filled)).toBe(5);
    expect(count(clear())).toBe(0);
  });

  it("selectAll selects every ordered id", () => {
    const all = selectAll(IDS);
    expect(orderedSelection(all, IDS)).toEqual(IDS);
  });
});

describe("selection — selectRange", () => {
  it("selects the inclusive range between anchor and target", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, "c2", "c4");
    expect(orderedSelection(s, IDS)).toEqual(["c2", "c3", "c4"]);
  });

  it("works when target precedes anchor (reverse drag)", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, "c4", "c2");
    expect(orderedSelection(s, IDS)).toEqual(["c2", "c3", "c4"]);
  });

  it("unions onto an existing selection", () => {
    const seed = toggle(EMPTY_SELECTION, "c1");
    const s = selectRange(seed, IDS, "c3", "c4");
    expect(orderedSelection(s, IDS)).toEqual(["c1", "c3", "c4"]);
  });

  it("falls back to toggle when there is no anchor", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, null, "c3");
    expect(orderedSelection(s, IDS)).toEqual(["c3"]);
  });

  it("falls back to toggle when an id is unknown", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, "ghost", "c3");
    expect(orderedSelection(s, IDS)).toEqual(["c3"]);
  });
});

describe("selection — orderedSelection", () => {
  it("returns selected ids in render order regardless of insertion order", () => {
    let s = toggle(EMPTY_SELECTION, "c4");
    s = toggle(s, "c1");
    s = toggle(s, "c3");
    expect(orderedSelection(s, IDS)).toEqual(["c1", "c3", "c4"]);
  });
});

describe("selection — isContiguous (mirrors backend merge rule)", () => {
  it("false for fewer than two captions", () => {
    expect(isContiguous(EMPTY_SELECTION, IDS)).toBe(false);
    expect(isContiguous(toggle(EMPTY_SELECTION, "c2"), IDS)).toBe(false);
  });

  it("true for two adjacent captions", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, "c2", "c3");
    expect(isContiguous(s, IDS)).toBe(true);
  });

  it("true for a longer adjacent run", () => {
    const s = selectRange(EMPTY_SELECTION, IDS, "c1", "c4");
    expect(isContiguous(s, IDS)).toBe(true);
  });

  it("false when there is a gap", () => {
    let s = toggle(EMPTY_SELECTION, "c1");
    s = toggle(s, "c3");
    expect(isContiguous(s, IDS)).toBe(false);
  });
});
