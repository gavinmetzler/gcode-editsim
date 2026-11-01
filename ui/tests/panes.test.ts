import { describe, expect, it } from "vitest";

import { layoutToSave, paneName, restoredShare, shownSeverities, splitFraction, toggleMaximised } from "../src/layout/panes";

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

describe("restoredShare", () => {
  it("gives back a remembered share that still fits", () => {
    expect(restoredShare(0.35, 1000, 100)).toBeCloseTo(0.35);
  });

  it("clamps a share that would make a pane too small in a smaller window", () => {
    // Saved on a wide screen: 5 % was 96 px of 1920; on 800 px that is 40.
    expect(restoredShare(0.05, 800, 100)).toBeCloseTo(0.125);
    expect(restoredShare(0.97, 800, 100)).toBeCloseTo(0.875);
  });

  it("gives the default (null) for nothing or nonsense", () => {
    for (const bad of [undefined, null, 0, 1, -0.5, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(restoredShare(bad, 1000, 100)).toBeNull();
    }
  });
});

describe("paneName", () => {
  it("accepts the pane names and nothing else", () => {
    expect(paneName("editor")).toBe("editor");
    expect(paneName("viewport")).toBe("viewport");
    expect(paneName("panel")).toBe("panel");
    for (const bad of [undefined, null, "", "toolbar", "Editor"]) expect(paneName(bad)).toBeNull();
  });
});

describe("layoutToSave", () => {
  const ALL = new Set(["error", "warning", "info"] as const);
  const base = { editor: null, panel: null, maximised: null, legendFolded: false, shown: ALL };

  it("leaves out what is at its default", () => {
    expect(layoutToSave(base)).toEqual({});
    expect(layoutToSave({ ...base, editor: 0.3, maximised: "panel" })).toEqual({ editor: 0.3, maximised: "panel" });
    expect(layoutToSave({ ...base, panel: 0.25 })).toEqual({ panel: 0.25 });
  });

  it("keeps a folded colour key and unticked filters, in a fixed order", () => {
    expect(layoutToSave({ ...base, legendFolded: true, shown: new Set(["warning"] as const) })).toEqual({
      legend_folded: true,
      hidden_diagnostics: ["error", "info"],
    });
  });
});

describe("shownSeverities", () => {
  it("shows everything not hidden", () => {
    expect(shownSeverities(undefined)).toEqual(new Set(["error", "warning", "info"]));
    expect(shownSeverities([])).toEqual(new Set(["error", "warning", "info"]));
    expect(shownSeverities(["info", "warning"])).toEqual(new Set(["error"]));
  });

  it("ignores names it does not know", () => {
    expect(shownSeverities(["notes", "Error", "info"])).toEqual(new Set(["error", "warning"]));
  });
});
