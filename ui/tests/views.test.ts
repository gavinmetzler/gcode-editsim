import { describe, expect, it } from "vitest";

import { MAX_POLAR, MIN_POLAR, VIEWS, fitOrtho, orbitAbout, orthoWorldPerPixel, type Vec3 } from "../src/viewport/views";

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

describe("orbitAbout", () => {
  const position: Vec3 = [100, -300, 250];
  const target: Vec3 = [20, 10, 0];
  const pivot: Vec3 = [-40, 60, -15];

  /** The pivot in the camera's own frame (right, up, forward), which fixes
   *  where it appears on screen with an orthographic camera. */
  function inCameraFrame(cam: { position: Vec3; target: Vec3 }, p: Vec3): number[] {
    const f = [cam.target[0] - cam.position[0], cam.target[1] - cam.position[1], cam.target[2] - cam.position[2]];
    const fl = Math.hypot(f[0] ?? 0, f[1] ?? 0, f[2] ?? 0);
    const fw = f.map((c) => c / fl);
    const [fx = 0, fy = 0, fz = 0] = fw;
    const rx = fy;
    const ry = -fx;
    const rl = Math.hypot(rx, ry);
    const right = [rx / rl, ry / rl, 0];
    const up = [ry / rl * fz - 0 * fy, 0 * fx - (rx / rl) * fz, (rx / rl) * fy - (ry / rl) * fx];
    const d = [p[0] - cam.position[0], p[1] - cam.position[1], p[2] - cam.position[2]];
    const dot = (a: number[], b: number[]) => (a[0] ?? 0) * (b[0] ?? 0) + (a[1] ?? 0) * (b[1] ?? 0) + (a[2] ?? 0) * (b[2] ?? 0);
    return [dot(d, right), dot(d, up), dot(d, fw)];
  }
  const polar = (cam: { position: Vec3; target: Vec3 }) => {
    const o = [cam.position[0] - cam.target[0], cam.position[1] - cam.target[1], cam.position[2] - cam.target[2]];
    return Math.acos((o[2] ?? 0) / Math.hypot(o[0] ?? 0, o[1] ?? 0, o[2] ?? 0));
  };

  it("does nothing for zero angles", () => {
    expect(orbitAbout(position, target, pivot, 0, 0)).toEqual({ position, target });
  });

  it("keeps the pivot where it is on screen", () => {
    const before = inCameraFrame({ position, target }, pivot);
    for (const [dt, dp] of [
      [0.7, 0],
      [0, 0.3],
      [-1.2, -0.4],
      [2.5, 0.2],
    ] as const) {
      const after = inCameraFrame(orbitAbout(position, target, pivot, dt, dp), pivot);
      after.forEach((c, k) => {
        expect(c).toBeCloseTo(before[k] ?? NaN, 6);
      });
    }
  });

  it("turns about the vertical through the pivot and tilts by dPhi", () => {
    const quarter = orbitAbout(position, target, pivot, Math.PI / 2, 0);
    // (100+40, -300-60) about the pivot, turned a quarter, is (360, 140).
    expect(quarter.position[0]).toBeCloseTo(-40 + 360, 6);
    expect(quarter.position[1]).toBeCloseTo(60 + 140, 6);
    expect(quarter.position[2]).toBeCloseTo(250, 6);
    const tilted = orbitAbout(position, target, pivot, 0, 0.2);
    expect(polar(tilted)).toBeCloseTo(polar({ position, target }) + 0.2, 9);
  });

  it("stops short of looking straight down or up", () => {
    expect(polar(orbitAbout(position, target, pivot, 0, -10))).toBeCloseTo(MIN_POLAR, 9);
    expect(polar(orbitAbout(position, target, pivot, 0, 10))).toBeCloseTo(MAX_POLAR, 9);
  });
});
