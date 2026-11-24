import { describe, expect, it } from "vitest";

import type { RenderBuffer } from "../src/buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../src/generated/render_buffer";
import {
  BACKGROUND,
  TOOL_PALETTE,
  HOVER_COLOURS,
  colourArray,
  contrastRatio,
  contrastingColour,
  deltaE,
  dimmed,
  displayColour,
  legendEntries,
  toolColour,
} from "../src/viewport/tool-colours";
import { filterToolpath } from "../src/viewport/tool-filter";
import { LEGEND, PALETTE, buildToolpath, cssColor, srgbToLinear, type Rgb } from "../src/viewport/toolpath";
import { source } from "./source";

const { Rapid, Linear, ArcCcw, ToolChange } = SEG_KIND;

// 0: rapid T1 (0-1), 1: linear T1 (2-3), 2: tool change T2 (4),
// 3: rapid T2 (5-6), 4: arc T2 (7-9), 5: linear T2 out of limits (10-11),
// 6: linear T2 from an unknown position (12-13).
const src = source(
  [0, 0, 0],
  [
    [Rapid, 1, 0, 0, [0, 0, 10], [0, 0, 1]],
    [Linear, 1, 100, 0, [0, 0, 1], [10, 0, 1]],
    [ToolChange, 2, 0, 0, [10, 0, 1]],
    [Rapid, 2, 0, 0, [10, 0, 1], [10, 0, 10]],
    [ArcCcw, 2, 100, 0, [10, 0, 1], [12, 2, 1], [10, 4, 1]],
    [Linear, 2, 100, SEG_FLAG.OUT_OF_LIMITS, [10, 4, 1], [99, 4, 1]],
    [Linear, 2, 100, SEG_FLAG.POSITION_UNKNOWN, [99, 4, 1], [99, 9, 1]],
  ],
);
const rb = {
  ...src,
  header: { ...src.header, vertexCount: src.positions.length / 3 },
  segmentLine: (i: number) => src.segment(i).srcLine,
} as unknown as RenderBuffer;
const tp = buildToolpath(rb);
const lin = (c: Rgb) => c.map(srgbToLinear).map((x) => expect.closeTo(x, 6) as unknown);
const at = (colors: Float32Array, v: number) => Array.from(colors.subarray(v * 3, v * 3 + 3));

describe("the tool palette (M5.5)", () => {
  it("has ten colours, each easy to tell from the others", () => {
    expect(TOOL_PALETTE).toHaveLength(10);
    for (let i = 0; i < TOOL_PALETTE.length; i += 1)
      for (let j = i + 1; j < TOOL_PALETTE.length; j += 1)
        expect(deltaE(TOOL_PALETTE[i] as Rgb, TOOL_PALETTE[j] as Rgb), `${String(i)}/${String(j)}`).toBeGreaterThan(25);
  });

  it("keeps clear of the colours with a fixed meaning: outside limits, unknown position, selected", () => {
    for (const c of TOOL_PALETTE)
      for (const fixed of [PALETTE.outOfLimits, PALETTE.positionUnknown, PALETTE.selected]) expect(deltaE(c, fixed)).toBeGreaterThan(35);
  });

  it("reads well on the dark background, and so do the dimmed rapids", () => {
    for (const c of TOOL_PALETTE) {
      expect(contrastRatio(c, BACKGROUND)).toBeGreaterThan(4.5);
      expect(contrastRatio(dimmed(c), BACKGROUND)).toBeGreaterThan(2);
      expect(deltaE(c, dimmed(c))).toBeGreaterThan(30);
    }
  });

  it("gives tools their colours in the order they are first used, repeating after ten", () => {
    expect(toolColour(0)).toBe(TOOL_PALETTE[0]);
    expect(toolColour(10)).toBe(TOOL_PALETTE[0]);
    expect(toolColour(13)).toBe(TOOL_PALETTE[3]);
  });
});

describe("colourArray (M5.5)", () => {
  it("is the toolpath's own colours when colouring by move", () => {
    expect(colourArray(tp, "move", [1, 2])).toBe(tp.colors);
  });

  it("colours by tool, rapids dimmed, the flags first", () => {
    const c = colourArray(tp, "tool", [1, 2]);
    const t1 = toolColour(0);
    const t2 = toolColour(1);
    expect(at(c, 0)).toEqual(lin(dimmed(t1))); // T1 rapid
    expect(at(c, 2)).toEqual(lin(t1)); // T1 linear
    expect(at(c, 5)).toEqual(lin(dimmed(t2))); // T2 rapid
    expect(at(c, 7)).toEqual(lin(t2)); // T2 arc
    expect(at(c, 10)).toEqual(lin(PALETTE.outOfLimits));
    expect(at(c, 12)).toEqual(lin(PALETTE.positionUnknown));
  });
});

describe("legendEntries (M5.5)", () => {
  it("is today's key when colouring by move", () => {
    expect(legendEntries("move", [1, 2])).toBe(LEGEND);
  });

  it("lists each tool with its colour when colouring by tool, and keeps the fixed colours", () => {
    const key = legendEntries("tool", [0, 1, 2]);
    const labels = key.map((e) => e.label);
    expect(labels.slice(0, 4)).toEqual(["Selected", "No tool", "T1", "T2"]);
    expect(key[2]?.color).toBe(cssColor(toolColour(1)));
    expect(labels).toContain("Rapids: dimmed tool colour");
    expect(labels).toContain("Outside travel limits");
    expect(labels).toContain("Position unknown");
    expect(labels).toContain("Tool change");
  });
});

describe("hiding rapids (M5.5, owner's addition)", () => {
  it("leaves out every rapid and the end points only rapids touch, keeping the feeds", () => {
    const f = filterToolpath(tp, new Set(), { rapids: false });
    expect(f.rapidIndex).toHaveLength(0);
    expect(f.feedIndex).toBe(tp.feedIndex);
    // A node stays while a feed ends or starts there (M6): a rapid's end
    // where a feed starts can still be hovered and measured to.
    const feed = (s: number): boolean => s >= 0 && tp.segmentKind[s] !== Rapid;
    for (let i = 0; i < tp.endpoints.segment.length; i += 1) {
      const kept = feed(tp.endpoints.segment[i] ?? -1) || feed(tp.endpoints.leaving[i] ?? -1);
      expect(f.endpointVisible[i], String(i)).toBe(kept ? 1 : 0);
    }
    // The first rapid ends where the first feed starts.
    expect(f.endpointVisible[1]).toBe(1);
  });

  it("works together with hidden tools", () => {
    const f = filterToolpath(tp, new Set([1]), { rapids: false });
    expect(f.rapidIndex).toHaveLength(0);
    // Only T2's feeds: the arc (7-9) and the two flagged lines.
    expect(Array.from(f.feedIndex)).toEqual([7, 8, 8, 9, 10, 11, 12, 13]);
  });
});

describe("displayColour (M6)", () => {
  it("is the colour each move is drawn in, by move or by tool", () => {
    for (const colouring of ["move", "tool"] as const) {
      const all = colourArray(tp, colouring, [1, 2]);
      for (let s = 0; s < tp.segmentKind.length; s += 1) {
        const v = tp.segmentVertices(s)?.first ?? 0;
        expect(at(all, v), `${colouring} ${String(s)}`).toEqual(lin(displayColour(tp, s, colouring, [1, 2])));
      }
    }
  });
});

describe("the hovered move's colour (M6)", () => {
  it("is never dark on the background, and never means selected or outside the limits", () => {
    for (const c of HOVER_COLOURS) {
      expect(contrastRatio(c, BACKGROUND)).toBeGreaterThan(7);
      for (const fixed of [PALETTE.selected, PALETTE.outOfLimits]) expect(deltaE(c, fixed)).toBeGreaterThan(40);
    }
  });

  it("contrasts the colour the move is drawn in, whatever it is", () => {
    const drawn = [PALETTE.rapid, PALETTE.linear, PALETTE.arc, PALETTE.outOfLimits, PALETTE.positionUnknown];
    for (const c of [...drawn, ...TOOL_PALETTE, ...TOOL_PALETTE.map(dimmed)])
      expect(deltaE(contrastingColour(c), c), cssColor(c)).toBeGreaterThan(40);
  });

  it("is the most different of its choices", () => {
    for (const c of [PALETTE.rapid, PALETTE.linear, PALETTE.arc]) {
      const best = Math.max(...HOVER_COLOURS.map((h) => deltaE(h, c)));
      expect(deltaE(contrastingColour(c), c)).toBe(best);
    }
  });
});
