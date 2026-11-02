import { describe, expect, it } from "vitest";

import { PACE, PacedKeys } from "../src/editor/paced-keys";

const plain = { shift: false, ctrl: false, alt: false, meta: false };
const shift = { ...plain, shift: true };

/** Ticks every 16 ms from `from` to `to`, collecting the steps. */
function run(k: PacedKeys, from: number, to: number): { at: number; key: string; extend: boolean }[] {
  const steps = [];
  for (let t = from; t <= to; t += 16) {
    const s = k.tick(t);
    if (s) steps.push({ at: t, ...s });
  }
  return steps;
}

describe("PacedKeys (M5.2)", () => {
  it("leaves a single press to the editor", () => {
    const k = new PacedKeys();
    expect(k.keyDown("PageDown", plain, false)).toBe(false);
    expect(k.active).toBe(false);
    expect(run(k, 0, 500)).toEqual([]);
  });

  it("takes over a held key: its repeats are ignored and it steps at its own pace", () => {
    const k = new PacedKeys();
    k.keyDown("PageDown", plain, false);
    // The keyboard repeats every 10 ms, faster than the page could follow.
    for (let i = 0; i < 100; i += 1) expect(k.keyDown("PageDown", plain, true)).toBe(true);
    expect(k.active).toBe(true);
    const steps = run(k, 1000, 2000);
    // About 30 a second, never closer than the pace allows.
    expect(steps.length).toBeGreaterThanOrEqual(25);
    expect(steps.length).toBeLessThanOrEqual(34);
    for (let i = 1; i < steps.length; i += 1) {
      expect((steps[i]?.at ?? 0) - (steps[i - 1]?.at ?? 0)).toBeGreaterThanOrEqual(PACE.minInterval);
    }
    expect(steps[0]).toEqual({ at: 1000, key: "PageDown", extend: false });
  });

  it("stops at once when the key is released: no steps are left to catch up", () => {
    const k = new PacedKeys();
    k.keyDown("ArrowDown", plain, false);
    k.keyDown("ArrowDown", plain, true);
    run(k, 0, 300);
    k.keyUp("ArrowDown");
    expect(k.active).toBe(false);
    expect(run(k, 316, 2000)).toEqual([]);
  });

  it("extends the selection while Shift is held, and follows Shift changing mid-hold", () => {
    const k = new PacedKeys();
    k.keyDown("PageUp", shift, false);
    k.keyDown("PageUp", shift, true);
    expect(k.tick(0)).toEqual({ key: "PageUp", extend: true });
    k.keyDown("PageUp", plain, true);
    expect(k.tick(100)).toEqual({ key: "PageUp", extend: false });
  });

  it("leaves other keys and combinations with Ctrl, Alt or Meta to the editor, and they stop pacing", () => {
    const k = new PacedKeys();
    k.keyDown("ArrowDown", plain, false);
    k.keyDown("ArrowDown", plain, true);
    expect(k.keyDown("ArrowDown", { ...plain, ctrl: true }, true)).toBe(false);
    expect(k.active).toBe(false);
    expect(k.keyDown("a", plain, true)).toBe(false);
    expect(k.keyDown("ArrowLeft", plain, true)).toBe(false);
  });

  it("switches to another held key", () => {
    const k = new PacedKeys();
    k.keyDown("ArrowDown", plain, false);
    k.keyDown("ArrowDown", plain, true);
    k.keyDown("ArrowUp", plain, true);
    expect(k.tick(0)).toEqual({ key: "ArrowUp", extend: false });
    // Releasing the earlier key does not stop the current one.
    k.keyUp("ArrowDown");
    expect(k.active).toBe(true);
  });

  it("does nothing when switched off, and stops pacing when switched off mid-hold", () => {
    const k = new PacedKeys();
    k.enabled = false;
    k.keyDown("PageDown", plain, false);
    expect(k.keyDown("PageDown", plain, true)).toBe(false);
    expect(k.active).toBe(false);
    k.enabled = true;
    k.keyDown("PageDown", plain, true);
    expect(k.active).toBe(true);
    k.enabled = false;
    expect(k.tick(0)).toBeNull();
    expect(k.active).toBe(false);
  });

  it("stop() ends a hold (the editor lost focus)", () => {
    const k = new PacedKeys();
    k.keyDown("PageDown", plain, true);
    k.stop();
    expect(k.active).toBe(false);
    expect(k.tick(0)).toBeNull();
  });

  it("paces at about 30 steps a second", () => {
    expect(PACE.minInterval).toBe(30);
  });
});
