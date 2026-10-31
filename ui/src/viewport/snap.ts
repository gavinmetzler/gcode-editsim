// Snapping the pointer to the end points of moves, in screen pixels. Pure,
// so it is tested without a GPU.
//
// Hover and clicks snap only to move end points (owner's request): finding
// the nearest of a few thousand projected points is far quicker than
// testing the pointer against every drawn line piece, of which arcs have
// many.

import type { RenderBuffer } from "../buffer/decode";
import { SEG_KIND } from "../generated/render_buffer";

/** How close (screen pixels) the pointer must be to snap to an end point. */
export const SNAP_PIXELS = 15;

/** The end points of the moves, relative to the buffer's origin. */
export interface Endpoints {
  /** x, y, z per end point, relative to the buffer's origin. */
  readonly positions: Float32Array;
  /** The move each end point belongs to. */
  readonly segment: Uint32Array;
  /** 1 where the point is the start of its move (no move ends there), 0 for
   *  the end of its move. */
  readonly isStart: Uint8Array;
}

const MOTION: ReadonlySet<number> = new Set([SEG_KIND.Rapid, SEG_KIND.Linear, SEG_KIND.ArcCw, SEG_KIND.ArcCcw]);

/**
 * The end of every move, and the start of a move where no move ends (the
 * first one, or after a gap such as an unknown position). Events (tool
 * changes, stops) are left out: they sit on move end points.
 */
export function moveEndpoints(
  rb: Pick<RenderBuffer, "positions" | "segment"> & { readonly header: Pick<RenderBuffer["header"], "segmentCount"> },
): Endpoints {
  const xyz: number[] = [];
  const segment: number[] = [];
  const isStart: number[] = [];
  const pos = rb.positions;
  let last: [number, number, number] | null = null;
  const at = (v: number): [number, number, number] => [pos[v * 3] ?? NaN, pos[v * 3 + 1] ?? NaN, pos[v * 3 + 2] ?? NaN];
  for (let s = 0; s < rb.header.segmentCount; s += 1) {
    const seg = rb.segment(s);
    if (!MOTION.has(seg.kind) || seg.vertexCount === 0) continue;
    const start = at(seg.firstVertex);
    const end = at(seg.firstVertex + seg.vertexCount - 1);
    if (!last || last[0] !== start[0] || last[1] !== start[1] || last[2] !== start[2]) {
      xyz.push(...start);
      segment.push(s);
      isStart.push(1);
    }
    xyz.push(...end);
    segment.push(s);
    isStart.push(0);
    last = end;
  }
  return {
    positions: Float32Array.from(xyz),
    segment: Uint32Array.from(segment),
    isStart: Uint8Array.from(isStart),
  };
}

/**
 * The index of the point in `screen` (x, y pairs in pixels; NaN for points
 * not on screen) nearest to (`x`, `y`), if it is within `tolerance` pixels;
 * otherwise -1. On a tie the later point wins, so where one move ends and
 * the next starts, the end is reported.
 */
export function nearestScreenPoint(screen: Float32Array, x: number, y: number, tolerance = SNAP_PIXELS): number {
  let best = -1;
  let bestSq = tolerance * tolerance;
  const n = screen.length / 2;
  for (let i = 0; i < n; i += 1) {
    const dx = (screen[i * 2] ?? NaN) - x;
    const dy = (screen[i * 2 + 1] ?? NaN) - y;
    const d = dx * dx + dy * dy;
    // NaN compares false, so points not on screen never win.
    if (d <= bestSq) {
      bestSq = d;
      best = i;
    }
  }
  return best;
}
