import { describe, expect, it } from "vitest";

import { CATCH_UP, behind } from "../src/catch-up";

describe("behind (M5.2)", () => {
  it("is behind while key presses keep arriving late", () => {
    expect(behind(500, 10)).toBe(true);
    expect(behind(CATCH_UP.lagLimit + 1, 0)).toBe(true);
  });

  it("keeps up when key presses arrive on time, however fast they come", () => {
    expect(behind(5, 0)).toBe(false);
    expect(behind(CATCH_UP.lagLimit, 0)).toBe(false);
  });

  it("has caught up once the late key presses stop coming", () => {
    expect(behind(5000, CATCH_UP.quiet)).toBe(false);
    expect(behind(5000, CATCH_UP.quiet + 100)).toBe(false);
  });

  it("is not behind before any key press", () => {
    expect(behind(0, Number.POSITIVE_INFINITY)).toBe(false);
  });

  it("allows a tenth of a second of lag, and waits a little longer than a key repeat to call it caught up", () => {
    expect(CATCH_UP.lagLimit).toBe(100);
    // Keyboards repeat every 25 to 50 ms.
    expect(CATCH_UP.quiet).toBeGreaterThan(50);
    expect(CATCH_UP.quiet).toBeLessThanOrEqual(200);
  });
});
