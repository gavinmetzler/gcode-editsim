import { describe, expect, it } from "vitest";

import { CATCH_UP, KeyHold, behind } from "../src/catch-up";

describe("behind (M5.2)", () => {
  it("is behind while key presses keep arriving late", () => {
    expect(behind(500, 10, 50)).toBe(true);
    expect(behind(CATCH_UP.lagLimit + 1, 0, 50)).toBe(true);
  });

  it("keeps up when key presses arrive on time, however fast they come", () => {
    expect(behind(5, 0, 50)).toBe(false);
    expect(behind(CATCH_UP.lagLimit, 0, 50)).toBe(false);
  });

  it("has caught up once the late key presses stop coming", () => {
    expect(behind(5000, CATCH_UP.quiet, 50)).toBe(false);
    expect(behind(5000, CATCH_UP.quiet + 100, 50)).toBe(false);
  });

  it("is not behind before any key press", () => {
    expect(behind(0, Number.POSITIVE_INFINITY, 50)).toBe(false);
  });

  it("never holds the highlight back when 3D draws are cheap: the delay is elsewhere", () => {
    // The owner's machine (2026-11-02): keys 20 s late, draws 1-2 ms.
    expect(behind(20_000, 0, 2)).toBe(false);
    expect(behind(20_000, 0, CATCH_UP.drawLimit)).toBe(false);
    expect(behind(20_000, 0, CATCH_UP.drawLimit + 1)).toBe(true);
  });

  it("allows a tenth of a second of lag, and waits a little longer than a key repeat to call it caught up", () => {
    expect(CATCH_UP.lagLimit).toBe(100);
    // Keyboards repeat every 25 to 50 ms.
    expect(CATCH_UP.quiet).toBeGreaterThan(50);
    expect(CATCH_UP.quiet).toBeLessThanOrEqual(200);
    // Half a 60 Hz frame.
    expect(CATCH_UP.drawLimit).toBe(8);
  });
});

describe("KeyHold (M5.2 diagnostics)", () => {
  it("summarises a held key: presses, the keyboard's time and the page's time", () => {
    const k = new KeyHold();
    // The keyboard sends a press every 40 ms for 2 s (its own timestamps);
    // the page handles them 100 ms apart.
    for (let i = 0; i <= 50; i += 1) k.keyDown(1000 + i * 40, 5000 + i * 100, i > 0);
    k.keyUp();
    expect(k.last).toEqual({ presses: 51, keyboardMs: 2000, handledMs: 5000 });
  });

  it("gives the keyboard's repeat interval, leaving out its initial repeat delay", () => {
    const k = new KeyHold();
    expect(k.keyDown(1000, 1000, false)).toBeNull();
    // The first repeat comes after the keyboard's repeat delay (600 ms here).
    expect(k.keyDown(1600, 1600, true)).toBeNull();
    expect(k.keyDown(1640, 1640, true)).toBe(40);
    expect(k.keyDown(1680, 1690, true)).toBe(40);
  });

  it("starts a new hold on a fresh press, and ignores a key-up without a hold", () => {
    const k = new KeyHold();
    k.keyUp();
    expect(k.last).toBeNull();
    k.keyDown(0, 0, false);
    k.keyUp();
    k.keyDown(100, 100, false);
    k.keyDown(140, 150, true);
    k.keyUp();
    expect(k.last).toEqual({ presses: 2, keyboardMs: 40, handledMs: 50 });
  });

  it("does not count a single press as a hold", () => {
    const k = new KeyHold();
    k.keyDown(0, 0, false);
    k.keyUp();
    expect(k.last).toBeNull();
  });
});
