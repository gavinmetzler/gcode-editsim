// Snapping to segment end points, in screen pixels. Pure, so it is tested
// without a GPU.

/** A point on screen, in pixels. */
export type ScreenPoint = readonly [number, number];

/** How close (screen pixels) the pointer must be to snap to an end point. */
export const SNAP_PIXELS = 10;

/**
 * Which end of a segment the pointer snaps to: the nearer of `start` and
 * `end` if it is within `tolerance` pixels, else null (the pointer stays on
 * the segment's path).
 */
export function snapEnd(
  pointer: ScreenPoint,
  start: ScreenPoint,
  end: ScreenPoint,
  tolerance = SNAP_PIXELS,
): "start" | "end" | null {
  const ds = Math.hypot(pointer[0] - start[0], pointer[1] - start[1]);
  const de = Math.hypot(pointer[0] - end[0], pointer[1] - end[1]);
  const [d, which] = de <= ds ? [de, "end" as const] : [ds, "start" as const];
  return d <= tolerance ? which : null;
}
