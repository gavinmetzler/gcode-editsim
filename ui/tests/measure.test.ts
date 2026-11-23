import { describe, expect, it } from "vitest";

import { deltas, measurePanel, planeDistance } from "../src/viewport/measure";

describe("deltas", () => {
  it("is end minus start, with the 3D distance", () => {
    expect(deltas([1, 2, 3], [4, 6, 15])).toEqual({ dx: 3, dy: 4, dz: 12, distance: 13 });
    expect(deltas([10, 0, 0], [-20, 0, 0])).toEqual({ dx: -30, dy: 0, dz: 0, distance: 30 });
  });
});

describe("measurePanel", () => {
  it("asks for a drag before any measurement", () => {
    expect(measurePanel(null)).toEqual({
      hint: "Drag from one node to another to measure.",
      rows: [],
      distance: null,
      planeDistance: null,
    });
  });

  it("shows the start while the drag has not reached a node", () => {
    const p = measurePanel({ start: [1, 2, 3], end: null });
    expect(p.rows).toEqual([{ label: "Start", values: ["1.000", "2.000", "3.000"] }]);
    expect(p.distance).toBeNull();
    expect(p.planeDistance).toBeNull();
    expect(p.hint).toBe("Drag to another node.");
  });

  it("shows start, end, deltas and distance", () => {
    const p = measurePanel({ start: [-380, -205, 0], end: [-330, -205, -402.5] });
    expect(p.rows).toEqual([
      { label: "Start", values: ["-380.000", "-205.000", "0.000"] },
      { label: "End", values: ["-330.000", "-205.000", "-402.500"] },
      { label: "Delta", values: ["50.000", "0.000", "-402.500"] },
    ]);
    expect(p.distance).toBe(`${Math.hypot(50, 402.5).toFixed(3)} mm`);
    expect(p.hint).toBeNull();
  });
});

describe("the distance in one plane (M6)", () => {
  it("leaves out the third axis", () => {
    expect(planeDistance([0, 0, 0], [3, 4, 12], "XY")).toBe(5);
    expect(planeDistance([0, 0, 0], [3, 4, 12], "XZ")).toBeCloseTo(Math.hypot(3, 12), 12);
    expect(planeDistance([0, 0, 0], [3, 4, 12], "YZ")).toBeCloseTo(Math.hypot(4, 12), 12);
  });

  it("is shown under the 3D distance, named for the machine's plane", () => {
    const m = { start: [0, 0, 0] as const, end: [30, 40, -5] as const };
    expect(measurePanel(m).planeDistance).toEqual({ label: "XY distance", value: "50.000 mm" });
    expect(measurePanel(m, "XZ").planeDistance).toEqual({
      label: "XZ distance",
      value: `${Math.hypot(30, 5).toFixed(3)} mm`,
    });
    expect(measurePanel(m).distance).toBe(`${Math.hypot(30, 40, 5).toFixed(3)} mm`);
  });
});
