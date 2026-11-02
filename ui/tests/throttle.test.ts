import { describe, expect, it } from "vitest";

import { Throttle, VIEW_UPDATES } from "../src/throttle";

/** A clock and timers the test moves by hand. */
function fakeTime() {
  let now = 0;
  let next = 1;
  const timers = new Map<number, { at: number; fn: () => void }>();
  return {
    now: () => now,
    set: (fn: () => void, ms: number): number => {
      const id = next++;
      timers.set(id, { at: now + ms, fn });
      return id;
    },
    clear: (id: number): void => {
      timers.delete(id);
    },
    pending: () => timers.size,
    /** Moves the clock to `to`, firing the timers due on the way, in order. */
    advance(to: number): void {
      for (;;) {
        const due = [...timers.entries()].filter(([, t]) => t.at <= to).sort((a, b) => a[1].at - b[1].at)[0];
        if (!due) break;
        timers.delete(due[0]);
        now = due[1].at;
        due[1].fn();
      }
      now = to;
    },
  };
}

function setup(timing = { interval: 500, settle: 100 }) {
  const time = fakeTime();
  const runs: number[] = [];
  const throttle = new Throttle(() => runs.push(time.now()), timing, time.now, { set: time.set, clear: time.clear });
  return { time, runs, throttle };
}

/** Requests at `every` ms from `from` to `to` (a held key repeating). */
function hold(s: ReturnType<typeof setup>, from: number, to: number, every: number): void {
  for (let t = from; t <= to; t += every) {
    s.time.advance(t);
    s.throttle.request();
  }
}

describe("Throttle", () => {
  it("updates at once after a pause, so a single key press or click is not delayed", () => {
    const s = setup();
    s.time.advance(1000);
    s.throttle.request();
    expect(s.runs).toEqual([1000]);
    expect(s.time.pending()).toBe(0);
  });

  it("updates about twice a second while a key is held", () => {
    const s = setup();
    hold(s, 0, 3000, 33);
    // One at once, then one every 500 ms: no more than seven in three seconds.
    expect(s.runs.length).toBeGreaterThanOrEqual(6);
    expect(s.runs.length).toBeLessThanOrEqual(7);
    for (let i = 1; i < s.runs.length; i++) {
      expect((s.runs[i] ?? 0) - (s.runs[i - 1] ?? 0)).toBeGreaterThanOrEqual(500);
    }
  });

  it("updates once more just after the last movement, so the view ends on the final line", () => {
    const s = setup();
    // The last repeat (2805) comes mid-interval: the next half-second
    // update would be at 3000, but the view updates 100 ms after the last
    // movement instead.
    hold(s, 0, 2805, 33);
    expect(s.runs.at(-1)).toBe(2500);
    s.time.advance(10_000);
    expect(s.runs.at(-1)).toBe(2905);
    expect(s.time.pending()).toBe(0);
  });

  it("always ends with an update at or after the last request, however the key is held", () => {
    for (let every = 10; every <= 60; every += 7) {
      for (let end = 300; end <= 2000; end += 97) {
        const s = setup();
        let last = 0;
        for (let t = 0; t <= end; t += every) {
          s.time.advance(t);
          s.throttle.request();
          last = t;
        }
        s.time.advance(end + 10_000);
        expect(s.runs.at(-1)).toBeGreaterThanOrEqual(last);
        expect((s.runs.at(-1) ?? 0) - last).toBeLessThanOrEqual(100);
      }
    }
  });

  it("never runs twice for one request, and not at all without one", () => {
    const s = setup();
    s.time.advance(5000);
    expect(s.runs).toEqual([]);
    s.throttle.request();
    s.time.advance(9000);
    expect(s.runs).toEqual([5000]);
  });

  it("waits for the movement to settle when a second press follows soon after", () => {
    const s = setup();
    s.throttle.request(); // runs at 0
    s.time.advance(200);
    s.throttle.request();
    expect(s.runs).toEqual([0]);
    s.time.advance(1000);
    expect(s.runs).toEqual([0, 300]);
  });

  it("flush runs a pending update now, and cancel drops it", () => {
    const s = setup();
    s.throttle.request();
    s.time.advance(50);
    s.throttle.request();
    s.throttle.flush();
    expect(s.runs).toEqual([0, 50]);
    expect(s.time.pending()).toBe(0);
    s.throttle.flush(); // nothing pending: no run
    expect(s.runs).toEqual([0, 50]);
    s.time.advance(60);
    s.throttle.request();
    s.throttle.cancel();
    s.time.advance(5000);
    expect(s.runs).toEqual([0, 50]);
  });

  it("uses a half-second interval for the 3D view, and settles faster than a key repeats slowly", () => {
    expect(VIEW_UPDATES.interval).toBe(500);
    // Keyboards repeat 20 to 40 times a second (25 to 50 ms apart): the
    // final update must not fire between two repeats of a held key.
    expect(VIEW_UPDATES.settle).toBeGreaterThan(50);
    expect(VIEW_UPDATES.settle).toBeLessThan(VIEW_UPDATES.interval);
  });
});
