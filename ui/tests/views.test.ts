import { describe, expect, it } from "vitest";

import { VIEWS, fitOrtho, orthoWorldPerPixel } from "../src/viewport/views";

describe("standard views", () => {
  it("look from unit directions, with Z up", () => {
    for (const v of Object.values(VIEWS)) {
      expect(Math.hypot(...v.from)).toBeCloseTo(1, 12);
      expect(v.up).toEqual([0, 0, 1]);
    }
    // Machine convention: top looks down Z (from just off vertical, towards
    // -Y, so Y is up on screen); front looks along +Y; right along -X.
    expect(VIEWS.top.from[2]).toBeCloseTo(1, 6);
    expect(VIEWS.top.from[1]).toBeLessThan(0);
    expect(VIEWS.front.from).toEqual([0, -1, 0]);
    expect(VIEWS.right.from).toEqual([1, 0, 0]);
  });
});

describe("fitOrtho", () => {
  it("places the camera back along the view direction, outside the sphere", () => {
    const fit = fitOrtho([10, 20, 30], 5, VIEWS.front, 1);
    expect(fit.position[0]).toBe(10);
    expect(fit.position[2]).toBe(30);
    expect(20 - fit.position[1]).toBeGreaterThan(5);
    expect(fit.far).toBeGreaterThan(20 - fit.position[1] + 5);
  });

  it("fits the narrower side with a margin", () => {
    expect(fitOrtho([0, 0, 0], 5, VIEWS.top, 2).halfHeight).toBeCloseTo(5.5, 12);
    // A tall window: the width (half-height times aspect) must still hold the sphere.
    const tall = fitOrtho([0, 0, 0], 5, VIEWS.top, 0.5);
    expect(tall.halfHeight * 0.5).toBeCloseTo(5.5, 12);
  });

  it("survives a zero or bad radius and aspect", () => {
    for (const [r, a] of [[0, 1], [-1, 1], [NaN, 1], [1, 0], [1, NaN]] as const) {
      expect(Number.isFinite(fitOrtho([0, 0, 0], r, VIEWS.iso, a).halfHeight)).toBe(true);
    }
  });
});

describe("orthoWorldPerPixel", () => {
  it("is the visible height over the pixel height, divided by zoom", () => {
    expect(orthoWorldPerPixel(100, 1, 200)).toBeCloseTo(1, 12);
    expect(orthoWorldPerPixel(100, 2, 200)).toBeCloseTo(0.5, 12);
    expect(Number.isFinite(orthoWorldPerPixel(100, 0, 0))).toBe(true);
  });
});
