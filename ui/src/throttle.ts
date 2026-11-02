/** Limits how often an expensive update runs while changes keep coming
 *  (M5.2): the first change after a pause runs it at once; while changes
 *  keep coming it runs at most once per `interval`; and it runs once more
 *  `settle` ms after the last change, so the result always shows the final
 *  state.
 *
 *  While a key is held (`setHeld`), gaps between changes do not count as
 *  stopping: a slow update delays the queued key repeats, which then
 *  arrive in bursts, and guessing "stopped" from the gaps made every burst
 *  update again. Instead it runs at most once per `interval`, and once
 *  more when the key is released. The update can read `held` to skip its
 *  expensive part until then. Used for the 3D view while the editor cursor moves (a held
 *  Page Up/Down redraws a million segments otherwise on every repeat). */

/** How often, in milliseconds. */
export interface ThrottleTiming {
  /** The shortest time between two updates while changes keep coming. */
  readonly interval: number;
  /** How long without a change counts as "stopped". Longer than the time
   *  between two repeats of a held key, or the final update fires while
   *  the key is still down. */
  readonly settle: number;
}

/** The 3D view, marker, readout and status follow the cursor about twice a
 *  second while it moves quickly (M5_PLAN §3). */
export const VIEW_UPDATES: ThrottleTiming = { interval: 500, settle: 100 };

/** Timers, passed in so tests can drive time by hand. */
export interface Timers {
  readonly set: (fn: () => void, ms: number) => number;
  readonly clear: (id: number) => void;
}

const browserTimers: Timers = {
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (id) => {
    window.clearTimeout(id);
  },
};

/** See the module comment. */
export class Throttle {
  private lastRun: number | null = null;
  private timer: number | null = null;
  private holding = false;
  private movedWhileHeld = false;

  constructor(
    private readonly run: () => void,
    private readonly timing: ThrottleTiming = VIEW_UPDATES,
    private readonly now: () => number = () => performance.now(),
    private readonly timers: Timers = browserTimers,
  ) {}

  /** True while a key is held (see `setHeld`). */
  get held(): boolean {
    return this.holding;
  }

  /** A key is held down and repeating (true), or was released (false). On
   *  release it updates at once if anything changed while the key was held. */
  setHeld(held: boolean): void {
    if (held) {
      if (!this.holding) this.movedWhileHeld = false;
      this.holding = true;
      return;
    }
    if (!this.holding) return;
    this.holding = false;
    const moved = this.movedWhileHeld || this.timer !== null;
    this.movedWhileHeld = false;
    if (!moved) return;
    this.cancel();
    this.fire(this.now());
  }

  /** Something changed: update now, or soon. */
  request(): void {
    const now = this.now();
    if (this.holding) {
      this.movedWhileHeld = true;
      if (this.timer !== null) return;
      const wait = this.lastRun === null ? 0 : this.lastRun + this.timing.interval - now;
      if (wait <= 0) this.fire(now);
      else this.schedule(wait);
      return;
    }
    if (this.timer === null && (this.lastRun === null || now - this.lastRun >= this.timing.interval)) {
      this.fire(now);
      return;
    }
    // Wait for the changes to settle, but no longer than the interval allows.
    const byInterval = (this.lastRun ?? now) + this.timing.interval;
    const at = Math.min(byInterval, now + this.timing.settle);
    if (this.timer !== null) this.timers.clear(this.timer);
    this.schedule(Math.max(0, at - now));
  }

  /** Runs a pending update now (nothing happens when none is pending). */
  flush(): void {
    if (this.timer === null) return;
    this.timers.clear(this.timer);
    this.timer = null;
    this.fire(this.now());
  }

  /** Drops a pending update. */
  cancel(): void {
    if (this.timer !== null) this.timers.clear(this.timer);
    this.timer = null;
  }

  private schedule(ms: number): void {
    this.timer = this.timers.set(() => {
      this.timer = null;
      this.fire(this.now());
    }, ms);
  }

  private fire(now: number): void {
    this.lastRun = now;
    this.run();
  }
}
