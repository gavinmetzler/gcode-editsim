// Pane sizes and maximising: pure functions over numbers and names, so
// they are tested without a DOM. layout/splitters.ts wires them to the
// page.

/** The panes the user can resize and maximise. */
export type PaneName = "editor" | "viewport" | "panel";

/** The smallest a pane may be dragged to, in pixels. */
export const MIN_PANE_PIXELS = 120;

/**
 * The share (0 to 1) of a `length`-pixel span that the first pane gets
 * when the splitter is dragged to `offset` pixels from the span's start.
 * Both panes keep at least `min` pixels; a span too small for that is
 * split in half. Never NaN, whatever the input.
 */
export function splitFraction(offset: number, length: number, min = MIN_PANE_PIXELS): number {
  if (!Number.isFinite(length) || length <= 0) return 0.5;
  if (length < 2 * min) return 0.5;
  const at = Number.isFinite(offset) ? offset : length / 2;
  const clamped = Math.min(Math.max(at, min), length - min);
  return clamped / length;
}

/** The maximised pane after a toggle on `pane`: maximises it, or restores
 *  the normal layout if it is the one already maximised. */
export function toggleMaximised(current: PaneName | null, pane: PaneName): PaneName | null {
  return current === pane ? null : pane;
}
