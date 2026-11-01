// A hand-made render-buffer source for tests: segments given as
// [kind, tool, feed, flags, ...vertices], vertices relative to `origin`,
// and optionally each segment's estimated time (NaN when not given).

import type { RenderBuffer, Segment } from "../src/buffer/decode";

/** What the pure UI modules read from a render buffer. */
export type Source = Pick<RenderBuffer, "positions" | "segment"> & {
  readonly header: Pick<RenderBuffer["header"], "segmentCount" | "origin">;
};

/** A source from segments given as [kind, tool, feed, flags, ...vertices],
 *  with `times[i]` as segment i's estimated time (NaN if missing). */
export function source(
  origin: [number, number, number],
  segments: [number, number, number, number, ...[number, number, number][]][],
  times: readonly number[] = [],
): Source {
  const positions: number[] = [];
  const table: Segment[] = [];
  for (const [kind, tool, feed, flags, ...vertices] of segments) {
    table.push({
      firstVertex: positions.length / 3,
      vertexCount: vertices.length,
      srcLine: table.length + 1,
      srcFile: 0,
      blockLow: table.length,
      tool,
      kind,
      flags,
      feed,
      time: times[table.length] ?? NaN,
    });
    for (const v of vertices) positions.push(...v);
  }
  return {
    header: { segmentCount: table.length, origin },
    positions: Float32Array.from(positions),
    segment: (i) => {
      const s = table[i];
      if (!s) throw new RangeError(String(i));
      return s;
    },
  };
}

