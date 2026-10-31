// Toolpath statistics, worked out from the render buffer alone: counts by
// segment kind, path lengths, bounds, tools and feed rates. Pure data, no
// DOM, so it is tested without a browser.
//
// No G-code meaning here: the core already turned the program into kinds,
// positions, tools and feeds; this only adds them up.

import type { RenderBuffer } from "../buffer/decode";
import { SEG_FLAG, SEG_KIND } from "../generated/render_buffer";

/** What the statistics need from a render buffer. */
export type StatsSource = Pick<RenderBuffer, "positions" | "segment"> & {
  readonly header: Pick<RenderBuffer["header"], "segmentCount" | "origin">;
};

/** A box in machine coordinates, mm. */
export interface Box {
  readonly min: readonly [number, number, number];
  readonly max: readonly [number, number, number];
}

/** Figures for one tool. Tool 0 means no tool was selected. */
export interface ToolStats {
  readonly tool: number;
  /** Length of feed moves (linear and arcs), mm. */
  readonly feedLength: number;
  /** Length of rapids, mm. */
  readonly rapidLength: number;
}

/** Statistics for one program run. */
export interface Stats {
  /** Number of segments of each `SEG_KIND` value that occurs. */
  readonly kindCounts: ReadonlyMap<number, number>;
  /** Length of feed moves (linear and arcs), mm. Arcs are measured along
   *  their drawn chords, so they are short by at most the arc tolerance's
   *  worth per chord. */
  readonly feedLength: number;
  /** Length of rapids, mm, as drawn (straight lines). */
  readonly rapidLength: number;
  /** Bounds of all motion, in machine coordinates. */
  readonly bounds: Box | null;
  /** Bounds of feed moves only, in machine coordinates. */
  readonly feedBounds: Box | null;
  /** Per tool, in order of first motion. */
  readonly tools: readonly ToolStats[];
  /** Lowest and highest feed rate of feed moves, mm/min (zero feeds are
   *  left out: they are already reported as diagnostics). */
  readonly feedRange: { readonly min: number; readonly max: number } | null;
  /** Segments flagged outside the machine's travel. */
  readonly outOfLimits: number;
  /** Segments drawn with an unknown position. */
  readonly positionUnknown: number;
}

class BoxBuilder {
  private readonly min = [Infinity, Infinity, Infinity];
  private readonly max = [-Infinity, -Infinity, -Infinity];
  private empty = true;

  add(x: number, y: number, z: number): void {
    const p = [x, y, z];
    for (let k = 0; k < 3; k += 1) {
      const c = p[k] ?? 0;
      this.min[k] = Math.min(this.min[k] ?? c, c);
      this.max[k] = Math.max(this.max[k] ?? c, c);
    }
    this.empty = false;
  }

  build(): Box | null {
    if (this.empty) return null;
    const [a = 0, b = 0, c = 0] = this.min;
    const [d = 0, e = 0, f = 0] = this.max;
    return { min: [a, b, c], max: [d, e, f] };
  }
}

/** Works out the statistics of `rb`. Non-finite positions are skipped. */
export function computeStats(rb: StatsSource): Stats {
  const kindCounts = new Map<number, number>();
  const tools = new Map<number, { feedLength: number; rapidLength: number }>();
  const all = new BoxBuilder();
  const feedOnly = new BoxBuilder();
  const [ox, oy, oz] = rb.header.origin;
  const pos = rb.positions;
  let feedLength = 0;
  let rapidLength = 0;
  let feedMin = Infinity;
  let feedMax = -Infinity;
  let outOfLimits = 0;
  let positionUnknown = 0;

  for (let s = 0; s < rb.header.segmentCount; s += 1) {
    const seg = rb.segment(s);
    kindCounts.set(seg.kind, (kindCounts.get(seg.kind) ?? 0) + 1);
    if ((seg.flags & SEG_FLAG.OUT_OF_LIMITS) !== 0) outOfLimits += 1;
    if ((seg.flags & SEG_FLAG.POSITION_UNKNOWN) !== 0) positionUnknown += 1;
    const rapid = seg.kind === SEG_KIND.Rapid;
    const feed =
      seg.kind === SEG_KIND.Linear || seg.kind === SEG_KIND.ArcCw || seg.kind === SEG_KIND.ArcCcw;
    if (!rapid && !feed) continue;

    let length = 0;
    let prev: [number, number, number] | null = null;
    for (let v = seg.firstVertex; v < seg.firstVertex + seg.vertexCount; v += 1) {
      const x = (pos[v * 3] ?? NaN) + ox;
      const y = (pos[v * 3 + 1] ?? NaN) + oy;
      const z = (pos[v * 3 + 2] ?? NaN) + oz;
      if (!(Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(z))) {
        prev = null;
        continue;
      }
      all.add(x, y, z);
      if (feed) feedOnly.add(x, y, z);
      if (prev) length += Math.hypot(x - prev[0], y - prev[1], z - prev[2]);
      prev = [x, y, z];
    }

    let perTool = tools.get(seg.tool);
    if (!perTool) {
      perTool = { feedLength: 0, rapidLength: 0 };
      tools.set(seg.tool, perTool);
    }
    if (rapid) {
      rapidLength += length;
      perTool.rapidLength += length;
    } else {
      feedLength += length;
      perTool.feedLength += length;
      if (seg.feed > 0 && Number.isFinite(seg.feed)) {
        feedMin = Math.min(feedMin, seg.feed);
        feedMax = Math.max(feedMax, seg.feed);
      }
    }
  }

  return {
    kindCounts,
    feedLength,
    rapidLength,
    bounds: all.build(),
    feedBounds: feedOnly.build(),
    tools: Array.from(tools, ([tool, t]) => ({ tool, ...t })),
    feedRange: feedMin <= feedMax ? { min: feedMin, max: feedMax } : null,
    outOfLimits,
    positionUnknown,
  };
}

/** Readable names for the segment kinds. */
export const KIND_LABELS = {
  Rapid: "Rapid",
  Linear: "Linear",
  ArcCw: "Arc CW",
  ArcCcw: "Arc CCW",
  Dwell: "Dwell",
  ToolChange: "Tool change",
  Sync: "Sync",
  Stop: "Stop",
  OptionalStop: "Optional stop",
  SpindleCw: "Spindle CW",
  SpindleCcw: "Spindle CCW",
  SpindleStop: "Spindle stop",
  CoolantFlood: "Coolant flood",
  CoolantMist: "Coolant mist",
  CoolantOff: "Coolant off",
  ProgramEnd: "Program end",
} as const satisfies Record<keyof typeof SEG_KIND, string>;

/** The label of `kind`, or its number if it is unknown. */
export function kindLabel(kind: number): string {
  for (const [name, value] of Object.entries(SEG_KIND)) {
    if (value === kind) return KIND_LABELS[name as keyof typeof SEG_KIND];
  }
  return `Kind ${String(kind)}`;
}

/** A length or coordinate in mm, to 3 decimals. */
export function mm(value: number): string {
  return `${value.toFixed(3)} mm`;
}

/** One row of the statistics table. */
export interface StatRow {
  readonly label: string;
  readonly value: string;
}

/** A titled group of rows. */
export interface StatSection {
  readonly title: string;
  readonly rows: readonly StatRow[];
}

/** A tool number as shown: tool 0 means none selected. */
export function toolName(tool: number): string {
  return tool === 0 ? "No tool" : tool === 0xffff ? "T65535 or above" : `T${String(tool)}`;
}

/** The statistics as titled groups of rows, in display order. Bounds are
 *  labelled with the render buffer's x, y and z, which on a mill are the
 *  X, Y and Z axes. */
export function statSections(stats: Stats): StatSection[] {
  const path: StatRow[] = [
    { label: "Feed length", value: mm(stats.feedLength) },
    { label: "Rapid length", value: mm(stats.rapidLength) },
  ];
  if (stats.feedRange) {
    const { min, max } = stats.feedRange;
    path.push({
      label: "Feed rate",
      value: min === max ? `${min.toFixed(1)} mm/min` : `${min.toFixed(1)} to ${max.toFixed(1)} mm/min`,
    });
  }
  const segments = (n: number): string => `${String(n)} segment${n === 1 ? "" : "s"}`;
  if (stats.outOfLimits > 0) path.push({ label: "Out of travel limits", value: segments(stats.outOfLimits) });
  if (stats.positionUnknown > 0) path.push({ label: "Position unknown", value: segments(stats.positionUnknown) });

  const bounds: StatRow[] = [];
  const box = (label: string, b: Box | null): void => {
    if (!b) return;
    (["X", "Y", "Z"] as const).forEach((axis, k) => {
      const lo = b.min[k] ?? 0;
      const hi = b.max[k] ?? 0;
      bounds.push({ label: `${label} ${axis}`, value: `${lo.toFixed(3)} to ${hi.toFixed(3)} (${mm(hi - lo)})` });
    });
  };
  box("Feed", stats.feedBounds);
  box("All", stats.bounds);

  const tools = stats.tools.map((t) => ({
    label: toolName(t.tool),
    value: `feed ${mm(t.feedLength)}, rapid ${mm(t.rapidLength)}`,
  }));
  const kinds = [...stats.kindCounts]
    .sort((a, b) => a[0] - b[0])
    .map(([kind, count]) => ({ label: kindLabel(kind), value: String(count) }));

  return [
    { title: "Path", rows: path },
    { title: "Bounds (machine coordinates)", rows: bounds },
    { title: "Tools", rows: tools },
    { title: "Segments", rows: kinds },
  ].filter((section) => section.rows.length > 0);
}
