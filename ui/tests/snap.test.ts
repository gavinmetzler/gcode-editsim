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

  it("skips events and the inside points of arcs", () => {
    expect(Array.from(e.segment)).not.toContain(1);
    expect(Array.from(e.positions)).not.toContain(44);
  });
});

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
