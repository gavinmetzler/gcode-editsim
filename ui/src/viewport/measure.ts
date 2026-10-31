// Measuring between two move end points: the deltas and the distance, and
// the rows the measurement panel shows. Pure, so it is tested without a
// browser.

import type { Vec3 } from "./views";

/** A measurement: a start node and, once the drag has reached one, an end
 *  node (machine coordinates, mm). */
export interface Measurement {
  readonly start: Vec3;
  readonly end: Vec3 | null;
}

/** End minus start on each axis, and the straight-line (3D) distance. */
export interface Deltas {
  readonly dx: number;
  readonly dy: number;
  readonly dz: number;
  readonly distance: number;
}

/** The deltas from `a` to `b`. */
export function deltas(a: Vec3, b: Vec3): Deltas {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const dz = b[2] - a[2];
  return { dx, dy, dz, distance: Math.hypot(dx, dy, dz) };
}

/** One row of the panel: a label and the X, Y and Z columns. */
export interface MeasureRow {
  readonly label: string;
  readonly values: readonly [string, string, string];
}

/** What the measurement panel shows: rows for the start, end and deltas,
 *  the distance, and a hint while there is nothing to show yet. */
export interface MeasurePanel {
  readonly hint: string | null;
  readonly rows: readonly MeasureRow[];
  readonly distance: string | null;
}

const mm = (v: number): string => v.toFixed(3);
const xyz = (p: Vec3): [string, string, string] => [mm(p[0]), mm(p[1]), mm(p[2])];

/** The panel for `m` (null before any measurement). */
export function measurePanel(m: Measurement | null): MeasurePanel {
  if (!m) return { hint: "Drag from one node to another to measure.", rows: [], distance: null };
  const rows: MeasureRow[] = [{ label: "Start", values: xyz(m.start) }];
  if (!m.end) return { hint: "Drag to another node.", rows, distance: null };
  const d = deltas(m.start, m.end);
  rows.push({ label: "End", values: xyz(m.end) }, { label: "Delta", values: [mm(d.dx), mm(d.dy), mm(d.dz)] });
  return { hint: null, rows, distance: `${mm(d.distance)} mm` };
}
