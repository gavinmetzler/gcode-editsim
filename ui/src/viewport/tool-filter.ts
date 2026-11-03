// Filtering the toolpath by tool (M5.4). Pure: from the drawing data and
// the hidden tools, the index lists and points to draw. Rebuilt only when
// the filter changes, never per frame, so it stays quick on large
// programs. Hidden tools' moves are not drawn, cannot be picked, snapped
// to or measured to, and the selection highlight leaves them out (it
// draws from the same index lists). The statistics always cover the whole
// program, and the hidden tools are not remembered (owner, M5_PLAN §6).

import type { Toolpath } from "./toolpath";

/** What to draw for one filter. */
export interface FilteredDrawing {
  readonly feedIndex: Uint32Array;
  readonly rapidIndex: Uint32Array;
  readonly eventPositions: Float32Array;
  readonly eventColors: Float32Array;
  readonly singlePoints: Toolpath["singlePoints"];
  /** 1 for each of the toolpath's end points that can be picked. */
  readonly endpointVisible: Uint8Array;
}

/** Every tool the program uses, once, in the order it is first used. */
export function toolsInOrder(segmentTool: ArrayLike<number>): number[] {
  const seen = new Set<number>();
  const tools: number[] = [];
  for (let s = 0; s < segmentTool.length; s += 1) {
    const t = segmentTool[s] ?? 0;
    if (!seen.has(t)) {
      seen.add(t);
      tools.push(t);
    }
  }
  return tools;
}

/** The drawing with the `hidden` tools left out. Without hidden tools, the
 *  toolpath's own arrays (no copy). */
export function filterToolpath(
  tp: Pick<
    Toolpath,
    | "feedIndex"
    | "rapidIndex"
    | "eventPositions"
    | "eventColors"
    | "eventSegment"
    | "singlePoints"
    | "segmentTool"
    | "vertexSegment"
    | "endpoints"
  >,
  hidden: ReadonlySet<number>,
): FilteredDrawing {
  const shown = (segment: number): boolean => !hidden.has(tp.segmentTool[segment] ?? 0);
  if (hidden.size === 0) {
    return {
      feedIndex: tp.feedIndex,
      rapidIndex: tp.rapidIndex,
      eventPositions: tp.eventPositions,
      eventColors: tp.eventColors,
      singlePoints: tp.singlePoints,
      endpointVisible: new Uint8Array(tp.endpoints.segment.length).fill(1),
    };
  }
  // Index pairs: both vertices of a pair belong to the same segment.
  const pairs = (index: Uint32Array): Uint32Array => {
    const out = new Uint32Array(index.length);
    let n = 0;
    for (let i = 0; i + 1 < index.length; i += 2) {
      const a = index[i] ?? 0;
      if (!shown(tp.vertexSegment[a] ?? 0)) continue;
      out[n] = a;
      out[n + 1] = index[i + 1] ?? 0;
      n += 2;
    }
    return out.slice(0, n);
  };
  // Points: `width` floats each, kept where their segment is shown.
  const points = (data: Float32Array, width: number, segments: ArrayLike<number>): Float32Array => {
    const out: number[] = [];
    for (let i = 0; i < segments.length; i += 1) {
      if (shown(segments[i] ?? 0)) for (let k = 0; k < width; k += 1) out.push(data[i * width + k] ?? 0);
    }
    return Float32Array.from(out);
  };
  const singles = tp.singlePoints;
  const keptSingles = Array.from(singles.segments).filter(shown);
  return {
    feedIndex: pairs(tp.feedIndex),
    rapidIndex: pairs(tp.rapidIndex),
    eventPositions: points(tp.eventPositions, 3, tp.eventSegment),
    eventColors: points(tp.eventColors, 3, tp.eventSegment),
    singlePoints: {
      positions: points(singles.positions, 3, singles.segments),
      lines: points(singles.lines, 1, singles.segments),
      segments: Uint32Array.from(keptSingles),
    },
    endpointVisible: Uint8Array.from(tp.endpoints.segment, (s) => (shown(s) ? 1 : 0)),
  };
}

/** The hidden tools that show only `tool` (right-click on it). */
export function onlyTool(tools: readonly number[], tool: number): Set<number> {
  return new Set(tools.filter((t) => t !== tool));
}

/** After a refresh: the hidden tools the program still has. */
export function keepHidden(hidden: ReadonlySet<number>, tools: readonly number[]): Set<number> {
  return new Set(tools.filter((t) => hidden.has(t)));
}

/** The toolbar button's label. */
export function toolsButtonLabel(total: number, hiddenCount: number): string {
  return hiddenCount === 0 ? "Tools ▾" : `Tools (${String(total - hiddenCount)} of ${String(total)}) ▾`;
}
