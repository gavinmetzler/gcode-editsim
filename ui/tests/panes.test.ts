import { describe, expect, it } from "vitest";

import { splitFraction, toggleMaximised } from "../src/layout/panes";

describe("splitFraction", () => {
  it("follows the pointer between the minimums", () => {
    expect(splitFraction(400, 1000, 100)).toBeCloseTo(0.4);
    expect(splitFraction(100, 1000, 100)).toBeCloseTo(0.1);
  });

  it("keeps both panes at least the minimum size", () => {
    expect(splitFraction(20, 1000, 100)).toBeCloseTo(0.1);
    expect(splitFraction(-500, 1000, 100)).toBeCloseTo(0.1);
    expect(splitFraction(990, 1000, 100)).toBeCloseTo(0.9);
  });

  it("halves a span too small or unknown", () => {
    expect(splitFraction(50, 150, 100)).toBe(0.5);
    expect(splitFraction(50, 0, 100)).toBe(0.5);
    expect(splitFraction(50, Number.NaN, 100)).toBe(0.5);
    expect(splitFraction(Number.NaN, 1000, 100)).toBe(0.5);
  });
});

describe("toggleMaximised", () => {
  it("maximises a pane, and a second toggle restores", () => {
    expect(toggleMaximised(null, "viewport")).toBe("viewport");
    expect(toggleMaximised("viewport", "viewport")).toBeNull();
  });

  it("switches straight to another pane", () => {
    expect(toggleMaximised("editor", "panel")).toBe("panel");
  });
});
