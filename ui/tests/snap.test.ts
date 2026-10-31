import { describe, expect, it } from "vitest";

import { SNAP_PIXELS, snapEnd } from "../src/viewport/snap";

describe("snapEnd", () => {
  it("snaps to the nearer end within the tolerance", () => {
    expect(snapEnd([3, 4], [0, 0], [100, 0], 10)).toBe("start");
    expect(snapEnd([95, 2], [0, 0], [100, 0], 10)).toBe("end");
  });

  it("stays on the path when both ends are too far", () => {
    expect(snapEnd([50, 1], [0, 0], [100, 0], 10)).toBeNull();
    expect(snapEnd([0, 10.5], [0, 0], [100, 0], 10)).toBeNull();
  });

  it("prefers the end when both are equally near (a short segment)", () => {
    expect(snapEnd([1, 0], [0, 0], [2, 0], 10)).toBe("end");
  });

  it("uses the default tolerance", () => {
    expect(snapEnd([SNAP_PIXELS, 0], [0, 0], [500, 0])).toBe("start");
    expect(snapEnd([SNAP_PIXELS + 0.1, 0], [0, 0], [500, 0])).toBeNull();
  });
});
