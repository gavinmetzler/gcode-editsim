// Whether the app is falling behind the keyboard (M5.2). Each key press
// says how long it waited before the page could handle it. When that is
// more than a tenth of a second, something (usually drawing frames) is
// slower than the keys repeat, and redrawing the 3D view only adds to the
// queue; the view then waits until the late key presses stop and updates
// once. On a machine that keeps up, the view follows every frame.

/** The limits, in milliseconds. */
export const CATCH_UP = {
  /** Key presses that waited longer than this mean the app is behind. */
  lagLimit: 100,
  /** No late key press for this long means it has caught up. Longer than
   *  the time between two repeats of a held key. */
  quiet: 150,
} as const;

/** True when the last key press waited `lastLag` ms and was handled
 *  `sinceLastKey` ms ago, and that means the app is still behind. */
export function behind(lastLag: number, sinceLastKey: number, limits = CATCH_UP): boolean {
  return lastLag > limits.lagLimit && sinceLastKey < limits.quiet;
}
