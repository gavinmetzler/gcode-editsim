import { describe, expect, it } from "vitest";

import { SEG_KIND } from "../src/generated/render_buffer";
import { SNAP_PIXELS, moveEndpoints, nearestScreenPoint } from "../src/viewport/snap";
import { source } from "./source";

const K = SEG_KIND;

describe("moveEndpoints", () => {
  // Rapid (0,0,50)→(0,0,10); tool change at (0,0,10); linear →(30,40,0);
  // arc through (33,44,0) →(36,40,0); a gap; linear (100,0,0)→(100,0,-5).
  const rb = source(
    [0, 0, 0],
    [
      [K.Rapid, 0, 0, 0, [0, 0, 50], [0, 0, 10]],
      [K.ToolChange, 3, 0, 0, [0, 0, 10]],
      [K.Linear, 3, 500, 0, [0, 0, 10], [30, 40, 0]],
      [K.ArcCcw, 3, 250, 0, [30, 40, 0], [33, 44, 0], [36, 40, 0]],
      [K.Linear, 3, 100, 0, [100, 0, 0], [100, 0, -5]],
    ],
  );
  const e = moveEndpoints(rb);

  it("lists each move's end once, plus starts where nothing ends", () => {
    const pts = Array.from({ length: e.segment.length }, (_, i) => [
      e.segment[i],
      e.isStart[i],
      ...Array.from(e.positions.subarray(i * 3, i * 3 + 3)),
    ]);
    expect(pts).toEqual([
      [0, 1, 0, 0, 50],
      [0, 0, 0, 0, 10],
      [2, 0, 30, 40, 0],
      [3, 0, 36, 40, 0],
      [4, 1, 100, 0, 0],
      [4, 0, 100, 0, -5],
    ]);
  });

  it("gives the direction of travel arriving at each end (M5.6g)", () => {
    // A start (nothing arrives) takes its own move's first direction; an
    // arc's end its last piece's direction.
    const dirs = Array.from({ length: e.segment.length }, (_, i) =>
      Array.from(e.direction.subarray(i * 3, i * 3 + 3)).map((v) => Math.round(v * 1000) / 1000),
    );
    expect(dirs).toEqual([[0, 0, -1], [0, 0, -1], unit([30, 40, -10]), [0.6, -0.8, 0], [0, 0, -1], [0, 0, -1]]);
  });

  it("has no direction for a move of zero length", () => {
    const z = moveEndpoints(source([0, 0, 0], [[K.Linear, 1, 100, 0, [5, 5, 5], [5, 5, 5]]]));
    expect(Array.from(z.direction).every((v) => Number.isNaN(v))).toBe(true);
  });

  it("skips repeated points when finding the direction", () => {
    const r = moveEndpoints(
      source([0, 0, 0], [[K.ArcCw, 1, 100, 0, [0, 0, 0], [0, 0, 0], [10, 0, 0], [10, 0, 0]]]),
    );
    expect(Array.from(r.direction)).toEqual([1, 0, 0, 1, 0, 0]);
  });

  it("skips events and the inside points of arcs", () => {
    expect(Array.from(e.segment)).not.toContain(1);
    expect(Array.from(e.positions)).not.toContain(44);
  });
});

function unit(v: [number, number, number]): number[] {
  const n = Math.hypot(...v);
  return v.map((c) => Math.round((c / n) * 1000) / 1000);
}

describe("nearestScreenPoint", () => {
  const screen = Float32Array.from([0, 0, 100, 0, 100.5, 0, NaN, NaN, 300, 300]);

  it("finds the nearest point within the tolerance", () => {
    expect(nearestScreenPoint(screen, 3, 4, 10)).toBe(0);
    expect(nearestScreenPoint(screen, 295, 300, 10)).toBe(4);
  });

  it("finds nothing farther than the tolerance", () => {
    expect(nearestScreenPoint(screen, 50, 0, 10)).toBe(-1);
    expect(nearestScreenPoint(screen, 0, SNAP_PIXELS + 0.1)).toBe(-1);
    expect(nearestScreenPoint(screen, 0, SNAP_PIXELS)).toBe(0);
  });

  it("prefers the later point on a tie, and never a point off screen", () => {
    expect(nearestScreenPoint(Float32Array.from([5, 0, 5, 0]), 5, 0)).toBe(1);
    expect(nearestScreenPoint(Float32Array.from([NaN, NaN]), 0, 0)).toBe(-1);
  });
});
