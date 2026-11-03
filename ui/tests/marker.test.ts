import { describe, expect, it } from "vitest";

import { MARKER, markerShape, tailOnScreen } from "../src/viewport/marker";

describe("markerShape", () => {
  it("points up the screen from the tip when Z is up in the view", () => {
    // Front view: +Z is straight up on screen (negative y), 2 px per mm.
    const s = markerShape(100, 200, 0, -2, 2);
    expect(s.kind).toBe("arrow");
    if (s.kind !== "arrow") return;
    expect(s.head.slice(0, 2)).toEqual([100, 200]);
    expect(s.shaft).toEqual([100, 200 - MARKER.headLength, 100, 200 - MARKER.length]);
    // The head's base is level, centred on the shaft.
    expect(s.head[3]).toBeCloseTo(200 - MARKER.headLength, 9);
    expect(s.head[5]).toBeCloseTo(200 - MARKER.headLength, 9);
    expect(Math.abs(s.head[2] - s.head[4])).toBeCloseTo(MARKER.headWidth, 9);
  });

  it("is shortened as Z tilts toward the viewer", () => {
    const s = markerShape(0, 0, 0, -1, 2); // half length
    expect(s.kind === "arrow" && s.shaft[3]).toBeCloseTo(-MARKER.length / 2, 9);
  });

  it("is a ring when looking along Z (Top view)", () => {
    expect(markerShape(5, 6, 0, -0.0002, 2)).toEqual({ kind: "ring", x: 5, y: 6, r: MARKER.ring });
    expect(markerShape(5, 6, 0, 0, 0).kind).toBe("ring");
  });
});

describe("tailOnScreen (M5.6g)", () => {
  // A Front view: X to the right, Z up, 2 px per mm on a 200 x 100 canvas
  // (column-major projection matrix: x' = X/50, y' = Z/25).
  const m = [1 / 50, 0, 0, 0, 0, 0, 0, 0, 0, 1 / 25, 0, 0, 0, 0, 0, 1];

  it("is +Z on screen with no direction (the vertical arrow)", () => {
    expect(tailOnScreen(null, m, 200, 100)).toEqual([0, -2]);
  });

  it("points back along the travel, so the arrow points along it", () => {
    // Travelling +X: the shaft trails to the left of the tip.
    expect(tailOnScreen([1, 0, 0], m, 200, 100)).toEqual([-2, 0]);
    const s = markerShape(100, 50, ...tailOnScreen([1, 0, 0], m, 200, 100), 2);
    expect(s.kind === "arrow" && s.shaft[2]).toBe(100 - MARKER.length);
  });

  it("falls back to +Z for a direction that is not a number", () => {
    expect(tailOnScreen([NaN, NaN, NaN], m, 200, 100)).toEqual([0, -2]);
  });

  it("is short when the travel is toward the viewer, so a ring is drawn", () => {
    // Travelling along Y (into the Front view's screen).
    const [tx, ty] = tailOnScreen([0, 1, 0], m, 200, 100);
    expect(markerShape(100, 50, tx, ty, 2).kind).toBe("ring");
  });
});
