import { describe, expect, it } from "vitest";

import { SEG_KIND } from "../src/generated/render_buffer";
import { SNAP_PIXELS, hoverMove, moveEndpoints, nearestScreenPoint } from "../src/viewport/snap";
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

  it("knows the move leaving each node, and its direction (M6)", () => {
    // The rapid's start: the rapid; its end: the linear (seg 2, the tool
    // change between is not a move); the linear's end: the arc; the arc's
    // end: none (the next move starts elsewhere); the gap's start: seg 4.
    expect(Array.from(e.leaving)).toEqual([0, 2, 3, -1, 4, -1]);
    const dirs = Array.from({ length: e.leaving.length }, (_, i) =>
      Array.from(e.leavingDirection.subarray(i * 3, i * 3 + 3)).map((v) => Math.round(v * 1000) / 1000),
    );
    expect(dirs[0]).toEqual([0, 0, -1]);
    expect(dirs[1]).toEqual(unit([30, 40, -10]));
    expect(dirs[2]).toEqual([0.6, 0.8, 0]);
    expect(dirs[3]?.every((v) => Number.isNaN(v))).toBe(true);
    expect(dirs[4]).toEqual([0, 0, -1]);
  });

  it("chooses the leaving move for hover, else the arriving one (M6)", () => {
    const all = (): boolean => true;
    expect(hoverMove(e, 1, all).segment).toBe(2);
    expect(Array.from(hoverMove(e, 2, all).direction).map((v) => Math.round(v * 1000) / 1000)).toEqual([0.6, 0.8, 0]);
    // Nothing leaves the arc's end: the arc itself, arriving.
    expect(hoverMove(e, 3, all)).toEqual({ segment: 3, direction: [0.6, -0.8, 0].map((v) => expect.closeTo(v, 6) as unknown) });
    // A leaving move that is not shown: the arriving one.
    expect(hoverMove(e, 1, (s) => s !== 2).segment).toBe(0);
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
