// Drawing data built from the shared fixture: rapid (vertices 0-1), spindle
// event (2), linear out of limits (3-4), CCW arc (5-7), program end with
// unknown position (8).

import { describe, expect, it } from "vitest";

import { decodeRenderBuffer } from "../src/buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../src/generated/render_buffer";
import {
  LEGEND,
  PALETTE,
  buildToolpath,
  cssColor,
  segmentColor,
  srgbToLinear,
  type Rgb,
} from "../src/viewport/toolpath";
import { fixture } from "./fixture";

const rb = decodeRenderBuffer(fixture());
const tp = buildToolpath(rb);

const colorOf = (v: number): number[] => Array.from(tp.colors.subarray(v * 3, v * 3 + 3));
const close = (a: readonly number[]) => a.map((x) => expect.closeTo(x, 6) as unknown);
const lin = (c: Rgb) => close(c.map(srgbToLinear));

describe("buildToolpath", () => {
  it("joins each segment's own vertices, never across segments", () => {
    expect(Array.from(tp.rapidIndex)).toEqual([0, 1]);
    expect(Array.from(tp.feedIndex)).toEqual([3, 4, 5, 6, 6, 7]);
  });

  it("maps every vertex to its segment", () => {
    expect(Array.from(tp.vertexSegment)).toEqual([0, 0, 1, 2, 2, 3, 3, 3, 4]);
  });

  it("draws events as markers at their position", () => {
    expect(Array.from(tp.eventSegment)).toEqual([1, 4]);
    expect(Array.from(tp.eventPositions)).toEqual(
      close([...rb.positions.subarray(6, 9), ...rb.positions.subarray(24, 27)]),
    );
  });

  it("colours by kind, with flags first", () => {
    expect(colorOf(0)).toEqual(lin(PALETTE.rapid));
    expect(colorOf(3)).toEqual(lin(PALETTE.outOfLimits));
    expect(colorOf(5)).toEqual(lin(PALETTE.arc));
    expect(colorOf(8)).toEqual(lin(PALETTE.positionUnknown));
    expect(Array.from(tp.eventColors.subarray(0, 3))).toEqual(lin(PALETTE.event));
  });

  it("bounds the motion only, relative to the origin", () => {
    expect(tp.bounds).toEqual({ min: close([-5, -5, -25.5]), max: close([5, 5, 25.5]) });
    expect(tp.origin).toEqual([1005, 2005, 24.5]);
  });

  it("shares the positions with the decoded buffer", () => {
    expect(tp.positions).toBe(rb.positions);
  });
});

describe("srgbToLinear", () => {
  it("matches the sRGB transfer function", () => {
    expect(srgbToLinear(0)).toBe(0);
    expect(srgbToLinear(1)).toBeCloseTo(1, 12);
    expect(srgbToLinear(0.5)).toBeCloseTo(0.214, 3);
    expect(srgbToLinear(0.04)).toBeCloseTo(0.04 / 12.92, 12);
  });
});

describe("segmentColor", () => {
  it("puts out-of-limits before position-unknown before kind", () => {
    const both = SEG_FLAG.OUT_OF_LIMITS | SEG_FLAG.POSITION_UNKNOWN;
    expect(segmentColor(SEG_KIND.Linear, both)).toBe(PALETTE.outOfLimits);
    expect(segmentColor(SEG_KIND.Stop, SEG_FLAG.POSITION_UNKNOWN)).toBe(PALETTE.positionUnknown);
    expect(segmentColor(SEG_KIND.ArcCw, 0)).toBe(PALETTE.arc);
    expect(segmentColor(SEG_KIND.ToolChange, 0)).toBe(PALETTE.toolChange);
    expect(segmentColor(SEG_KIND.ProgramEnd, 0)).toBe(PALETTE.programEnd);
    expect(segmentColor(SEG_KIND.CoolantFlood, 0)).toBe(PALETTE.event);
  });
});

describe("legend", () => {
  it("converts palette colours to CSS", () => {
    expect(cssColor([1, 0, 0.5])).toBe("rgb(255 0 128)");
    expect(cssColor([2, -1, 0])).toBe("rgb(255 0 0)");
  });

  it("names every colour segmentColor can give", () => {
    const shown = new Set(LEGEND.map((e) => e.color));
    const kinds = Object.values(SEG_KIND);
    const flags = [0, SEG_FLAG.OUT_OF_LIMITS, SEG_FLAG.POSITION_UNKNOWN];
    for (const k of kinds) {
      for (const f of flags) expect(shown.has(cssColor(segmentColor(k, f))), `kind ${String(k)} flags ${String(f)}`).toBe(true);
    }
  });

  it("says out-of-limits is about the travel limits", () => {
    expect(LEGEND.find((e) => e.color === cssColor(PALETTE.outOfLimits))?.label).toBe("Outside travel limits");
  });
});
