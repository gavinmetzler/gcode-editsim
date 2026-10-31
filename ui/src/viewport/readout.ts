// What the tool marker points at, and the readout beside it: position,
// tool, feed and source line. Pure functions over the render buffer, so
// they are tested without a GPU. No G-code meaning: kinds, tools and feeds
// are the core's.

import type { RenderBuffer, Segment } from "../buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../generated/render_buffer";
import { kindLabel, toolName } from "../panels/stats";
import type { Vec3 } from "./views";

/** What the render buffer must offer. */
export type ReadoutSource = Pick<RenderBuffer, "positions" | "segment"> & {
  readonly header: Pick<RenderBuffer["header"], "segmentCount" | "origin">;
};

/** Where on a segment a point lies. */
export type Place = "start" | "end" | "path" | "event";

/** A point on the toolpath and the segment it belongs to. */
export interface ToolPoint {
  /** Machine coordinates, mm. */
  readonly point: Vec3;
  readonly segment: number;
  readonly place: Place;
}

const MOTION: ReadonlySet<number> = new Set([SEG_KIND.Rapid, SEG_KIND.Linear, SEG_KIND.ArcCw, SEG_KIND.ArcCcw]);

/** Segments looked back over (events at the same spot) to find the move
 *  that ends where another starts. */
const LOOK_BACK = 16;

function vertex(rb: ReadoutSource, v: number): Vec3 {
  const [ox, oy, oz] = rb.header.origin;
  return [
    (rb.positions[v * 3] ?? NaN) + ox,
    (rb.positions[v * 3 + 1] ?? NaN) + oy,
    (rb.positions[v * 3 + 2] ?? NaN) + oz,
  ];
}

/** The last vertex of segment `s` (its end, or an event's position), in
 *  machine coordinates; null for a segment without vertices. */
export function segmentEnd(rb: ReadoutSource, s: number): Vec3 | null {
  const seg = rb.segment(s);
  return seg.vertexCount === 0 ? null : vertex(rb, seg.firstVertex + seg.vertexCount - 1);
}

/**
 * A point snapped to the start of segment `s` is also the end of the move
 * before it, and that move's feed and tool are what brought the tool
 * there. Returns that move's end if a motion segment ends at the same
 * point (looking back over events there), else the start as given.
 */
export function preferArrivingMove(rb: ReadoutSource, p: ToolPoint): ToolPoint {
  if (p.place !== "start") return p;
  const seg = rb.segment(p.segment);
  if (seg.vertexCount === 0) return p;
  const [sx, sy, sz] = vertex(rb, seg.firstVertex);
  for (let q = p.segment - 1; q >= 0 && q >= p.segment - LOOK_BACK; q -= 1) {
    const end = segmentEnd(rb, q);
    if (!end) continue;
    if (end[0] !== sx || end[1] !== sy || end[2] !== sz) break;
    if (MOTION.has(rb.segment(q).kind)) return { point: end, segment: q, place: "end" };
  }
  return p;
}

/**
 * Where the tool is after source `line` has run: the end of the last
 * segment from that line or before it. `lastSegmentBefore` is the index of
 * that segment (from the line lookup), or -1 if none. Null before the first
 * segment.
 */
export function restingPoint(rb: ReadoutSource, lastSegmentBefore: number): ToolPoint | null {
  for (let s = lastSegmentBefore; s >= 0; s -= 1) {
    const end = segmentEnd(rb, s);
    if (end) return { point: end, segment: s, place: MOTION.has(rb.segment(s).kind) ? "end" : "event" };
  }
  return null;
}

/** One line of the readout. */
export interface ReadoutRow {
  readonly label: string;
  readonly value: string;
}

/** The feed as shown: "Rapid" for rapids, none for events. */
export function feedText(seg: Segment): string {
  if (seg.kind === SEG_KIND.Rapid) return "Rapid";
  if (!MOTION.has(seg.kind)) return "–";
  return seg.feed > 0 && Number.isFinite(seg.feed) ? `${seg.feed.toFixed(1)} mm/min` : "none";
}

/** Where the point is, in words. */
export function placeText(seg: Segment, place: Place): string {
  const kind = kindLabel(seg.kind);
  switch (place) {
    case "start":
      return `Start of ${kind} move`;
    case "end":
      return `End of ${kind} move`;
    case "path":
      return `On ${kind} move`;
    case "event":
      return kind;
  }
}

/** The readout for `p`. Positions are labelled with the render buffer's
 *  x, y and z, which on a mill are the X, Y and Z axes. */
export function readoutRows(rb: ReadoutSource, p: ToolPoint): ReadoutRow[] {
  const seg = rb.segment(p.segment);
  const rows: ReadoutRow[] = (["X", "Y", "Z"] as const).map((axis, k) => ({
    label: axis,
    value: (p.point[k] ?? NaN).toFixed(3),
  }));
  rows.push(
    { label: "Tool", value: toolName(seg.tool) },
    { label: "Feed", value: feedText(seg) },
    { label: "Line", value: String(seg.srcLine) },
    { label: "At", value: placeText(seg, p.place) },
  );
  if ((seg.flags & SEG_FLAG.POSITION_UNKNOWN) !== 0) rows.push({ label: "Note", value: "Position unknown" });
  if ((seg.flags & SEG_FLAG.OUT_OF_LIMITS) !== 0) rows.push({ label: "Note", value: "Outside travel limits" });
  return rows;
}
