// Turns a decoded render buffer into what the viewport draws: index lists
// for feed moves and rapids, per-vertex colours, event markers and a
// vertex-to-segment map for picking. Pure data, no three.js, so it is
// tested without a GPU.
//
// The UI gives kinds no G-code meaning: it only maps the core's segment
// kinds and flags to colours.

import type { RenderBuffer } from "../buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../generated/render_buffer";

/** A colour as 0-1 components. */
export type Rgb = readonly [number, number, number];

/** One sRGB component (0-1) in linear light, as the renderer expects. */
export function srgbToLinear(c: number): number {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Colours by segment kind, and for flagged segments, in sRGB. */
export const PALETTE = {
  rapid: [0.95, 0.6, 0.15],
  linear: [0.25, 0.6, 1.0],
  arc: [0.3, 0.85, 0.45],
  outOfLimits: [1.0, 0.15, 0.15],
  event: [0.85, 0.85, 0.85],
  stop: [1.0, 0.85, 0.2],
  toolChange: [0.8, 0.45, 1.0],
  programEnd: [1.0, 0.3, 0.6],
  positionUnknown: [0.5, 0.5, 0.5],
} as const satisfies Record<string, Rgb>;

const MOTION_KINDS: ReadonlySet<number> = new Set([
  SEG_KIND.Rapid,
  SEG_KIND.Linear,
  SEG_KIND.ArcCw,
  SEG_KIND.ArcCcw,
]);

/** The colour for a segment of `kind` with `flags`. */
export function segmentColor(kind: number, flags: number): Rgb {
  if ((flags & SEG_FLAG.OUT_OF_LIMITS) !== 0) return PALETTE.outOfLimits;
  if ((flags & SEG_FLAG.POSITION_UNKNOWN) !== 0) return PALETTE.positionUnknown;
  switch (kind) {
    case SEG_KIND.Rapid:
      return PALETTE.rapid;
    case SEG_KIND.Linear:
      return PALETTE.linear;
    case SEG_KIND.ArcCw:
    case SEG_KIND.ArcCcw:
      return PALETTE.arc;
    case SEG_KIND.Stop:
    case SEG_KIND.OptionalStop:
      return PALETTE.stop;
    case SEG_KIND.ToolChange:
      return PALETTE.toolChange;
    case SEG_KIND.ProgramEnd:
      return PALETTE.programEnd;
    default:
      return PALETTE.event;
  }
}

/** Everything the viewport needs to draw one render buffer. */
export interface Toolpath {
  /** Vertex positions (x, y, z relative to `origin`), shared with the buffer. */
  readonly positions: Float32Array;
  /** Pairs of vertex indices for feed moves (linear and arcs). */
  readonly feedIndex: Uint32Array;
  /** Pairs of vertex indices for rapids. */
  readonly rapidIndex: Uint32Array;
  /** r, g, b per vertex, in linear light. */
  readonly colors: Float32Array;
  /** Event markers: x, y, z each, relative to `origin`. */
  readonly eventPositions: Float32Array;
  /** r, g, b per event marker, in linear light. */
  readonly eventColors: Float32Array;
  /** The segment of each event marker. */
  readonly eventSegment: Uint32Array;
  /** The segment each vertex belongs to. */
  readonly vertexSegment: Uint32Array;
  /** Where vertex coordinates are measured from (machine coordinates, mm). */
  readonly origin: readonly [number, number, number];
  /** The vertices of segment `index`, or null if there is no such segment. */
  segmentVertices(index: number): { readonly first: number; readonly count: number } | null;
  /** Bounds of the motion vertices relative to `origin`, or null if none. */
  readonly bounds: { readonly min: Rgb; readonly max: Rgb } | null;
}

function linear(c: Rgb): Rgb {
  return [srgbToLinear(c[0]), srgbToLinear(c[1]), srgbToLinear(c[2])];
}

/** Builds the drawing data for `rb`. */
export function buildToolpath(rb: RenderBuffer): Toolpath {
  const { vertexCount, segmentCount } = rb.header;
  const colors = new Float32Array(vertexCount * 3);
  const vertexSegment = new Uint32Array(vertexCount);
  const feed: number[] = [];
  const rapid: number[] = [];
  const events: number[] = [];
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];

  for (let s = 0; s < segmentCount; s += 1) {
    const seg = rb.segment(s);
    const color = linear(segmentColor(seg.kind, seg.flags));
    const end = seg.firstVertex + seg.vertexCount;
    for (let v = seg.firstVertex; v < end; v += 1) {
      vertexSegment[v] = s;
      colors.set(color, v * 3);
    }
    if (!MOTION_KINDS.has(seg.kind)) {
      if (seg.vertexCount > 0) events.push(s);
      continue;
    }
    const pairs = seg.kind === SEG_KIND.Rapid ? rapid : feed;
    for (let v = seg.firstVertex; v + 1 < end; v += 1) {
      pairs.push(v, v + 1);
    }
    for (let v = seg.firstVertex; v < end; v += 1) {
      for (let k = 0; k < 3; k += 1) {
        const c = rb.positions[v * 3 + k] ?? 0;
        if (Number.isFinite(c)) {
          min[k] = Math.min(min[k] ?? c, c);
          max[k] = Math.max(max[k] ?? c, c);
        }
      }
    }
  }

  const eventPositions = new Float32Array(events.length * 3);
  const eventColors = new Float32Array(events.length * 3);
  events.forEach((s, i) => {
    const seg = rb.segment(s);
    eventPositions.set(rb.positions.subarray(seg.firstVertex * 3, seg.firstVertex * 3 + 3), i * 3);
    eventColors.set(linear(segmentColor(seg.kind, seg.flags)), i * 3);
  });

  const hasBounds = min.every((m) => Number.isFinite(m));
  return {
    positions: rb.positions,
    feedIndex: Uint32Array.from(feed),
    rapidIndex: Uint32Array.from(rapid),
    colors,
    eventPositions,
    eventColors,
    eventSegment: Uint32Array.from(events),
    vertexSegment,
    origin: rb.header.origin,
    segmentVertices: (index: number) => {
      if (!Number.isInteger(index) || index < 0 || index >= segmentCount) return null;
      const s = rb.segment(index);
      return { first: s.firstVertex, count: s.vertexCount };
    },
    bounds: hasBounds
      ? {
          min: [min[0] ?? 0, min[1] ?? 0, min[2] ?? 0],
          max: [max[0] ?? 0, max[1] ?? 0, max[2] ?? 0],
        }
      : null,
  };
}
