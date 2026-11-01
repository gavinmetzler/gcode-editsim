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
  /** Estimated time, seconds: its moves, tool changes and dwells, leaving
   *  out segments without a time. */
  readonly time: number;
}

/** The program's estimated time, seconds, from the core's per-segment
 *  times (no acceleration). */
export interface TimeStats {
  /** Feed moves (linear and arcs). */
  readonly feed: number;
  /** Rapids. */
  readonly rapid: number;
  /** Everything else: tool changes and dwells (other events take none). */
  readonly other: number;
  /** All of the above. */
  readonly total: number;
  /** Segments the core could not estimate, left out of the sums. */
  readonly unknown: number;
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
  /** The time estimate. */
  readonly time: TimeStats;
  /** Per tool, in order of first motion or timed event. */
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
  const tools = new Map<number, { feedLength: number; rapidLength: number; time: number }>();
  const toolEntry = (tool: number) => {
    let t = tools.get(tool);
    if (!t) {
      t = { feedLength: 0, rapidLength: 0, time: 0 };
      tools.set(tool, t);
    }
    return t;
  };
  const time = { feed: 0, rapid: 0, other: 0, unknown: 0 };
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
    if (!Number.isFinite(seg.time) || seg.time < 0) {
      time.unknown += 1;
    } else if (rapid || feed || seg.time > 0) {
      // Events of no time do not give their tool a row.
      if (rapid) time.rapid += seg.time;
      else if (feed) time.feed += seg.time;
      else time.other += seg.time;
      toolEntry(seg.tool).time += seg.time;
    }
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

    const perTool = toolEntry(seg.tool);
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
    time: { ...time, total: time.feed + time.rapid + time.other },
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

/** A time in seconds as shown: seconds to one decimal under a minute,
 *  then minutes and seconds, then hours, minutes and seconds. */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "unknown";
  if (seconds < 59.95) return `${seconds.toFixed(1)} s`;
  const whole = Math.round(seconds);
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = String(whole % 60).padStart(2, "0");
  return h > 0 ? `${String(h)} h ${String(m).padStart(2, "0")} min ${s} s` : `${String(m)} min ${s} s`;
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
  const time: StatRow[] = [
    { label: "Total", value: duration(stats.time.total) },
    { label: "Feed moves", value: duration(stats.time.feed) },
    { label: "Rapids", value: duration(stats.time.rapid) },
    { label: "Tool changes and dwells", value: duration(stats.time.other) },
  ];
  if (stats.time.unknown > 0) time.push({ label: "Not estimated", value: segments(stats.time.unknown) });
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
    value: `feed ${mm(t.feedLength)}, rapid ${mm(t.rapidLength)}, time ${duration(t.time)}`,
  }));
  const kinds = [...stats.kindCounts]
    .sort((a, b) => a[0] - b[0])
    .map(([kind, count]) => ({ label: kindLabel(kind), value: String(count) }));

  return [
    { title: "Time (estimate)", rows: time },
    { title: "Path", rows: path },
    { title: "Bounds (machine coordinates)", rows: bounds },
    { title: "Tools", rows: tools },
    { title: "Segments", rows: kinds },
  ].filter((section) => section.rows.length > 0);
}
