import { describe, expect, it } from "vitest";

import type { RenderBuffer } from "../src/buffer/decode";
import { SEG_KIND } from "../src/generated/render_buffer";
import { filterToolpath, keepHidden, onlyTool, toolsButtonLabel, toolsInOrder } from "../src/viewport/tool-filter";
import { buildToolpath } from "../src/viewport/toolpath";
import { source } from "./source";

const { Rapid, Linear, ArcCcw, ToolChange } = SEG_KIND;

// Segments (index: kind, tool, vertices):
// 0: rapid, no tool yet          vertices 0-1
// 1: tool change to T1 (event)   vertex 2
// 2: linear, T1                  vertices 3-4
// 3: arc, T1                     vertices 5-7
// 4: tool change to T2 (event)   vertex 8
// 5: linear, T2                  vertices 9-10
// 6: rapid, T1 again             vertices 11-12
const src = source(
  [0, 0, 0],
  [
    [Rapid, 0, 0, 0, [0, 0, 10], [0, 0, 5]],
    [ToolChange, 1, 0, 0, [0, 0, 5]],
    [Linear, 1, 100, 0, [0, 0, 5], [10, 0, 5]],
    [ArcCcw, 1, 100, 0, [10, 0, 5], [12, 2, 5], [10, 4, 5]],
    [ToolChange, 2, 0, 0, [10, 4, 5]],
    [Linear, 2, 100, 0, [10, 4, 5], [20, 4, 5]],
    [Rapid, 1, 0, 0, [20, 4, 5], [20, 4, 10]],
  ],
);
const rb = {
  ...src,
  header: { ...src.header, vertexCount: src.positions.length / 3 },
  segmentLine: (i: number) => src.segment(i).srcLine,
} as unknown as RenderBuffer;
const tp = buildToolpath(rb);

describe("toolsInOrder (M5.4)", () => {
  it("lists every tool in the program once, in the order it is first used", () => {
    expect(toolsInOrder(tp.segmentTool)).toEqual([0, 1, 2]);
  });

  it("is empty for an empty program", () => {
    expect(toolsInOrder(new Uint16Array())).toEqual([]);
  });
});

describe("filterToolpath (M5.4)", () => {
  it("draws everything, without copying, when no tool is hidden", () => {
    const f = filterToolpath(tp, new Set());
    expect(f.feedIndex).toBe(tp.feedIndex);
    expect(f.rapidIndex).toBe(tp.rapidIndex);
    expect(f.eventPositions).toBe(tp.eventPositions);
    expect(f.singlePoints).toBe(tp.singlePoints);
    expect(Array.from(f.endpointVisible).every((v) => v === 1)).toBe(true);
  });

  it("leaves out a hidden tool's moves, events, highlight dots and end points", () => {
    const f = filterToolpath(tp, new Set([1]));
    // Feeds: only T2's linear (9-10); rapids: only the first (no tool).
    expect(Array.from(f.feedIndex)).toEqual([9, 10]);
    expect(Array.from(f.rapidIndex)).toEqual([0, 1]);
    // Events: T2's tool change only (T1's is hidden), with its colour.
    expect(Array.from(f.eventPositions)).toEqual([10, 4, 5]);
    expect(f.eventColors).toHaveLength(3);
    expect(Array.from(f.singlePoints.lines)).toEqual([src.segment(4).srcLine]);
    // End points of T1's moves cannot be picked or snapped to.
    for (let i = 0; i < tp.endpoints.segment.length; i += 1) {
      const tool = tp.segmentTool[tp.endpoints.segment[i] ?? 0];
      expect(f.endpointVisible[i]).toBe(tool === 1 ? 0 : 1);
    }
  });

  it("draws nothing when every tool is hidden", () => {
    const f = filterToolpath(tp, new Set([0, 1, 2]));
    expect(f.feedIndex).toHaveLength(0);
    expect(f.rapidIndex).toHaveLength(0);
    expect(f.eventPositions).toHaveLength(0);
    expect(f.singlePoints.positions).toHaveLength(0);
    expect(Array.from(f.endpointVisible).every((v) => v === 0)).toBe(true);
  });
});

describe("the tool filter's controls (M5.4)", () => {
  it("shows only the chosen tool (right-click)", () => {
    expect([...onlyTool([0, 1, 2], 1)].sort()).toEqual([0, 2]);
  });

  it("keeps hidden only the tools the program still has after a refresh", () => {
    expect([...keepHidden(new Set([1, 7]), [0, 1, 2])]).toEqual([1]);
  });

  it("says on the button how many tools are shown, when some are hidden", () => {
    expect(toolsButtonLabel(3, 0)).toBe("Tools ▾");
    expect(toolsButtonLabel(3, 1)).toBe("Tools (2 of 3) ▾");
    expect(toolsButtonLabel(3, 3)).toBe("Tools (0 of 3) ▾");
    expect(toolsButtonLabel(0, 0)).toBe("Tools ▾");
  });
});
