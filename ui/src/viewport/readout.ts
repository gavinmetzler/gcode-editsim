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
  /** The move the hover marker shows and highlights (M6): the one leaving
   *  the point (else the one arriving), and the unit direction the tool
   *  travels along it there. Only points under the pointer have one. */
  readonly hover?: { readonly segment: number; readonly direction: Vec3 };
}

const MOTION: ReadonlySet<number> = new Set([SEG_KIND.Rapid, SEG_KIND.Linear, SEG_KIND.ArcCw, SEG_KIND.ArcCcw]);

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
 * The point as the panel describes it and a click goes to its line: by the
 * move the hover highlights (M6, owner), so a node where one move ends and
 * the next starts reads as the start of the next move, with its line, feed
 * and tool. A point whose highlighted move is its own, or that has none,
 * is returned as it is. The hover move is kept, for the marker.
 */
export function shownMove(p: ToolPoint): ToolPoint {
  const hover = p.hover;
  if (!hover || hover.segment === p.segment) return p;
  return { point: p.point, segment: hover.segment, place: "start", hover };
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

/** A point the user clicked, and the source line the click went to. */
export interface Pinned {
  readonly line: number;
  readonly point: ToolPoint;
}

/**
 * Where the marker rests with the editor cursor on `line`: at the clicked
 * point while the cursor is still on the line the click went to, else at
 * `lineEnd` (where the tool is after the line, from `restingPoint`).
 */
export function restingFor(pinned: Pinned | null, line: number, lineEnd: ToolPoint | null): ToolPoint | null {
  return pinned?.line === line ? pinned.point : lineEnd;
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
