// Whether the app is falling behind the keyboard (M5.2). Each key press
// says how long it waited before the page could handle it. When that is
// more than a tenth of a second and the 3D draws are expensive (software
// rendering), redrawing the view only adds to the queue, so the highlight
// waits until the late key presses stop and updates once. When the draws
// are cheap (1-2 ms on the owner's machine), holding the view back would
// only hide the selection, so it follows every frame. Under Wayland, GTK
// gives a held key's repeats the first press's timestamp, so their
// waiting time is not real; held cursor keys are paced by the editor
// instead (paced-keys.ts).

/** The limits, in milliseconds. */
export const CATCH_UP = {
  /** Key presses that waited longer than this mean the app is behind. */
  lagLimit: 100,
  /** No late key press for this long means it has caught up. Longer than
   *  the time between two repeats of a held key. */
  quiet: 150,
  /** A 3D draw taking longer than this (half a 60 Hz frame) is expensive. */
  drawLimit: 8,
} as const;

/** True when the highlight should wait: the last key press waited
 *  `lastLag` ms, was handled `sinceLastKey` ms ago, and the last 3D draw
 *  took `drawMs`. */
export function behind(lastLag: number, sinceLastKey: number, drawMs: number, limits = CATCH_UP): boolean {
  return lastLag > limits.lagLimit && sinceLastKey < limits.quiet && drawMs > limits.drawLimit;
}

/** One held key, for the diagnostics panel. */
export interface HoldSummary {
  /** Key presses, the first and its repeats. */
  readonly presses: number;
  /** From the first to the last press, by the keyboard's timestamps. */
  readonly keyboardMs: number;
  /** From handling the first press to handling the last, in the page. */
  readonly handledMs: number;
}

/** Follows key presses to tell how fast the keyboard repeats and whether
 *  the page handles them as fast as they are sent. */
export class KeyHold {
  private presses = 0;
  private firstStamp = 0;
  private lastStamp = 0;
  private firstHandled = 0;
  private lastHandled = 0;
  /** The last finished hold of more than one press. */
  last: HoldSummary | null = null;

  /** A key press with the keyboard's timestamp `stamp`, handled at `now`.
   *  Returns the time since the previous press by the keyboard's clock for
   *  the second and later repeats, else null. */
  keyDown(stamp: number, now: number, repeat: boolean): number | null {
    if (!repeat || this.presses === 0) {
      this.presses = 1;
      this.firstStamp = stamp;
      this.lastStamp = stamp;
      this.firstHandled = now;
      this.lastHandled = now;
      return null;
    }
    // The first repeat follows the keyboard's repeat delay, not its rate.
    const interval = this.presses >= 2 ? stamp - this.lastStamp : null;
    this.presses += 1;
    this.lastStamp = stamp;
    this.lastHandled = now;
    return interval;
  }

  /** The key was released. */
  keyUp(): void {
    if (this.presses > 1) {
      this.last = {
        presses: this.presses,
        keyboardMs: this.lastStamp - this.firstStamp,
        handledMs: this.lastHandled - this.firstHandled,
      };
    }
    this.presses = 0;
  }
}
