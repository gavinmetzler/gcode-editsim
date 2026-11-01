// Statistics from a hand-made source (worked out by hand below) and from
// the shared fixture.

import { describe, expect, it } from "vitest";

import { decodeRenderBuffer } from "../src/buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../src/generated/render_buffer";
import { KIND_LABELS, computeStats, duration, kindLabel, mm, statSections, toolName } from "../src/panels/stats";
import { fixture } from "./fixture";
import { source } from "./source";

const K = SEG_KIND;
// Origin (100, 200, 0): vertices are relative to it.
const program = source(
  [100, 200, 0],
  [
    [K.Rapid, 0, 0, 0, [0, 0, 50], [0, 0, 10]], // 40 rapid, no tool
    [K.ToolChange, 3, 0, 0, [0, 0, 10]],
    [K.Rapid, 3, 0, 0, [0, 0, 10], [30, 40, 10]], // 50 rapid
    [K.Linear, 3, 500, 0, [30, 40, 10], [30, 40, 0]], // 10 feed
    [K.ArcCcw, 3, 250, SEG_FLAG.OUT_OF_LIMITS, [30, 40, 0], [33, 44, 0], [36, 40, 0]], // 5 + 5 feed
    [K.Linear, 7, 0, SEG_FLAG.POSITION_UNKNOWN, [36, 40, 0], [36, 40, -2]], // 2 feed, zero feed rate
    [K.ProgramEnd, 7, 0, 0, [36, 40, -2]],
  ],
  // Times in seconds; the zero-feed move has none (NaN).
  [0.5, 8, 0.3, 1.2, 2.4, NaN, 0],
);

describe("computeStats", () => {
  const stats = computeStats(program);

  it("adds up feed and rapid lengths along the drawn vertices", () => {
    expect(stats.rapidLength).toBeCloseTo(90, 9);
    expect(stats.feedLength).toBeCloseTo(22, 9);
  });

  it("gives bounds in machine coordinates, for all motion and feed moves only", () => {
    expect(stats.bounds).toEqual({ min: [100, 200, -2], max: [136, 244, 50] });
    expect(stats.feedBounds).toEqual({ min: [130, 240, -2], max: [136, 244, 10] });
  });

  it("splits lengths by tool, in order of first motion", () => {
    expect(stats.tools.map((t) => t.tool)).toEqual([0, 3, 7]);
    expect(stats.tools[0]).toMatchObject({ rapidLength: 40, feedLength: 0 });
    expect(stats.tools[1]?.rapidLength).toBeCloseTo(50, 9);
    expect(stats.tools[1]?.feedLength).toBeCloseTo(20, 9);
    expect(stats.tools[2]?.feedLength).toBeCloseTo(2, 9);
  });

  it("adds up the estimated times by kind and by tool, counting the unknown ones", () => {
    expect(stats.time.rapid).toBeCloseTo(0.8, 9);
    expect(stats.time.feed).toBeCloseTo(3.6, 9);
    expect(stats.time.other).toBeCloseTo(8, 9);
    expect(stats.time.total).toBeCloseTo(12.4, 9);
    expect(stats.time.unknown).toBe(1);
    expect(stats.tools.map((t) => t.time)).toEqual([0.5, expect.closeTo(11.9, 9), 0]);
  });

  it("takes the feed range from feed moves, leaving out zero feeds", () => {
    expect(stats.feedRange).toEqual({ min: 250, max: 500 });
  });

  it("counts kinds and flags", () => {
    expect([...stats.kindCounts]).toEqual([
      [K.Rapid, 2],
      [K.ToolChange, 1],
      [K.Linear, 2],
      [K.ArcCcw, 1],
      [K.ProgramEnd, 1],
    ]);
    expect(stats.outOfLimits).toBe(1);
    expect(stats.positionUnknown).toBe(1);
  });

  it("skips non-finite positions without breaking the rest", () => {
    const s = computeStats(
      source(
        [0, 0, 0],
        [[K.Linear, 1, 100, 0, [0, 0, 0], [NaN, 0, 0], [3, 4, 0], [6, 8, 0]]],
      ),
    );
    expect(s.feedLength).toBeCloseTo(5, 9);
    expect(s.bounds).toEqual({ min: [0, 0, 0], max: [6, 8, 0] });
  });

  it("is empty for a program without motion", () => {
    const s = computeStats(source([0, 0, 0], [[K.ProgramEnd, 0, 0, 0, [0, 0, 0]]]));
    expect(s).toMatchObject({ feedLength: 0, rapidLength: 0, bounds: null, feedBounds: null, feedRange: null });
    expect(s.tools).toEqual([]);
  });

  it("reads the shared fixture", () => {
    const s = computeStats(decodeRenderBuffer(fixture()));
    // Bounds of the fixture's motion: origin (1005, 2005, 24.5) ± (5, 5, 25.5).
    expect(s.bounds?.min.map((c) => Math.round(c * 1000) / 1000)).toEqual([1000, 2000, -1]);
    expect(s.bounds?.max.map((c) => Math.round(c * 1000) / 1000)).toEqual([1010, 2010, 50]);
    expect(s.outOfLimits).toBe(1);
    expect(s.positionUnknown).toBe(1);
    // 0.125 + 0 + 1.44 + 1.875 s (as f32); the program end has no time.
    expect(s.time.total).toBeCloseTo(3.44, 5);
    expect(s.time.unknown).toBe(1);
  });
});

describe("statSections", () => {
  it("lists path, bounds, tools and segment counts", () => {
    const sections = statSections(computeStats(program));
    expect(sections.map((s) => s.title)).toEqual([
      "Time (estimate)",
      "Path",
      "Bounds (machine coordinates)",
      "Tools",
      "Segments",
    ]);
    expect(sections[0]?.rows).toEqual([
      { label: "Total", value: "12.4 s" },
      { label: "Feed moves", value: "3.6 s" },
      { label: "Rapids", value: "0.8 s" },
      { label: "Tool changes and dwells", value: "8.0 s" },
      { label: "Not estimated", value: "1 segment" },
    ]);
    const path = sections[1]?.rows ?? [];
    expect(path).toContainEqual({ label: "Feed length", value: "22.000 mm" });
    expect(path).toContainEqual({ label: "Feed rate", value: "250.0 to 500.0 mm/min" });
    expect(path).toContainEqual({ label: "Out of travel limits", value: "1 segment" });
    expect(sections[2]?.rows[0]).toEqual({ label: "Feed X", value: "130.000 to 136.000 (6.000 mm)" });
    expect(sections[3]?.rows.map((r) => r.label)).toEqual(["No tool", "T3", "T7"]);
    expect(sections[3]?.rows[1]?.value).toBe("feed 20.000 mm, rapid 50.000 mm, time 11.9 s");
    expect(sections[4]?.rows).toContainEqual({ label: "Arc CCW", value: "1" });
  });

  it("leaves out empty sections", () => {
    const sections = statSections(computeStats(source([0, 0, 0], [])));
    expect(sections.map((s) => s.title)).toEqual(["Time (estimate)", "Path"]);
  });
});

describe("labels", () => {
  it("names every segment kind, and unknown ones by number", () => {
    for (const [name, value] of Object.entries(SEG_KIND)) {
      expect(kindLabel(value)).toBe(KIND_LABELS[name as keyof typeof SEG_KIND]);
    }
    expect(kindLabel(99)).toBe("Kind 99");
  });

  it("formats tools and lengths", () => {
    expect(toolName(0)).toBe("No tool");
    expect(toolName(12)).toBe("T12");
    expect(toolName(65535)).toBe("T65535 or above");
    expect(mm(1.23456)).toBe("1.235 mm");
  });

  it("formats durations", () => {
    expect(duration(0)).toBe("0.0 s");
    expect(duration(42.25)).toBe("42.3 s");
    expect(duration(59.96)).toBe("1 min 00 s");
    expect(duration(754.4)).toBe("12 min 34 s");
    expect(duration(3725)).toBe("1 h 02 min 05 s");
    expect(duration(Number.NaN)).toBe("unknown");
  });
});
