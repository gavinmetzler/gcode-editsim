// Readout and marker placement over a hand-made program (origin 100, 200, 0):
//   0 rapid (0,0,50) → (0,0,10), no tool       line 1
//   1 tool change T3 at (0,0,10)                line 2
//   2 spindle CW at (0,0,10)                    line 3
//   3 linear F500 (0,0,10) → (30,40,0)          line 4
//   4 arc CCW F250 (30,40,0) … (36,40,0), out of limits   line 5
//   5 linear, feed 0, position unknown          line 6

import { describe, expect, it } from "vitest";

import { SEG_FLAG, SEG_KIND } from "../src/generated/render_buffer";
import {
  feedText,
  placeText,
  preferArrivingMove,
  readoutRows,
  restingFor,
  restingPoint,
  segmentEnd,
} from "../src/viewport/readout";
import { source } from "./source";

const K = SEG_KIND;
const rb = source(
  [100, 200, 0],
  [
    [K.Rapid, 0, 0, 0, [0, 0, 50], [0, 0, 10]],
    [K.ToolChange, 3, 0, 0, [0, 0, 10]],
    [K.SpindleCw, 3, 0, 0, [0, 0, 10]],
    [K.Linear, 3, 500, 0, [0, 0, 10], [30, 40, 0]],
    [K.ArcCcw, 3, 250, SEG_FLAG.OUT_OF_LIMITS, [30, 40, 0], [33, 44, 0], [36, 40, 0]],
    [K.Linear, 3, 0, SEG_FLAG.POSITION_UNKNOWN, [36, 40, 0], [36, 40, -2]],
  ],
);
const value = (rows: { label: string; value: string }[], label: string) => rows.find((r) => r.label === label)?.value;

describe("segmentEnd and restingPoint", () => {
  it("gives a segment's last vertex in machine coordinates", () => {
    expect(segmentEnd(rb, 4)).toEqual([136, 240, 0]);
    expect(segmentEnd(rb, 1)).toEqual([100, 200, 10]);
  });

  it("rests the marker at the end of the line's last segment", () => {
    expect(restingPoint(rb, 3)).toEqual({ point: [130, 240, 0], segment: 3, place: "end" });
    expect(restingPoint(rb, 1)).toEqual({ point: [100, 200, 10], segment: 1, place: "event" });
  });

  it("has no resting point before the first segment", () => {
    expect(restingPoint(rb, -1)).toBeNull();
  });
});

describe("preferArrivingMove", () => {
  it("reports the start of a move as the end of the move that arrived there, past events", () => {
    const p = preferArrivingMove(rb, { point: [100, 200, 10], segment: 3, place: "start" });
    expect(p).toEqual({ point: [100, 200, 10], segment: 0, place: "end" });
  });

  it("keeps the start when nothing ends there", () => {
    const first = { point: [100, 200, 50] as const, segment: 0, place: "start" as const };
    expect(preferArrivingMove(rb, first)).toBe(first);
  });

  it("leaves other places alone", () => {
    const mid = { point: [115, 220, 5] as const, segment: 3, place: "path" as const };
    expect(preferArrivingMove(rb, mid)).toBe(mid);
  });
});

describe("readoutRows", () => {
  it("shows position, tool, feed, line and place", () => {
    const rows = readoutRows(rb, { point: [130, 240, 0], segment: 3, place: "end" });
    expect(rows.slice(0, 3)).toEqual([
      { label: "X", value: "130.000" },
      { label: "Y", value: "240.000" },
      { label: "Z", value: "0.000" },
    ]);
    expect(value(rows, "Tool")).toBe("T3");
    expect(value(rows, "Feed")).toBe("500.0 mm/min");
    expect(value(rows, "Line")).toBe("4");
    expect(value(rows, "At")).toBe("End of Linear move");
  });

  it("notes flags", () => {
    const arc = readoutRows(rb, { point: [133, 244, 0], segment: 4, place: "path" });
    expect(arc.map((r) => r.value)).toContain("Outside travel limits");
    expect(value(arc, "At")).toBe("On Arc CCW move");
    const unknown = readoutRows(rb, { point: [136, 240, -2], segment: 5, place: "end" });
    expect(unknown.map((r) => r.value)).toContain("Position unknown");
  });

  it("names rapids, events and missing feeds", () => {
    expect(feedText(rb.segment(0))).toBe("Rapid");
    expect(feedText(rb.segment(1))).toBe("–");
    expect(feedText(rb.segment(5))).toBe("none");
    expect(value(readoutRows(rb, { point: [100, 200, 10], segment: 0, place: "end" }), "Tool")).toBe("No tool");
    expect(placeText(rb.segment(1), "event")).toBe("Tool change");
  });
});

describe("restingFor", () => {
  const clicked = { point: [115, 220, 5] as const, segment: 3, place: "path" as const };
  const lineEnd = { point: [130, 240, 0] as const, segment: 3, place: "end" as const };

  it("rests at the clicked point while the cursor is on the clicked line", () => {
    expect(restingFor({ line: 4, point: clicked }, 4, lineEnd)).toBe(clicked);
  });

  it("rests at the end of the line once the cursor is on another line", () => {
    expect(restingFor({ line: 4, point: clicked }, 5, lineEnd)).toBe(lineEnd);
    expect(restingFor(null, 4, lineEnd)).toBe(lineEnd);
    expect(restingFor(null, 4, null)).toBeNull();
  });
});
