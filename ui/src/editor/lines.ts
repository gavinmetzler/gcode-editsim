// Mapping between editor lines and render-buffer segments and diagnostics.
// Pure functions over numbers, tested without an editor or a GPU.

import type { Diagnostic } from "../buffer/decode";
import { SEVERITY } from "../generated/render_buffer";

/** Which segments each source line produced, for programs whose lines
 *  run more than once (subprograms) as well as straight-line ones. */
export interface LineIndex {
  /** The segments from `line`, in run order: every run of the line. */
  segments(line: number): Uint32Array;
  /** The segments from every line between `first` and `last` (either
   *  order, both included): all their runs, by line. A view of the index,
   *  not a copy, so a range as large as the program costs nothing. */
  segmentsInLines(first: number, last: number): Uint32Array;
  /**
   * The segment that leaves the tool where it is after `line`: the last
   * segment of the line's first run, or, for a line with no segments, of
   * the nearest earlier line that has some. -1 before the first segment.
   */
  lastAfter(line: number): number;
}

/** Builds the index for `count` segments, `lineOf` giving each one's line. */
export function buildLineIndex(count: number, lineOf: (index: number) => number): LineIndex {
  const lines = new Uint32Array(count);
  for (let i = 0; i < count; i += 1) lines[i] = lineOf(i);
  // Segment indices sorted by line, then by run order.
  const order = Uint32Array.from({ length: count }, (_, i) => i).sort(
    (a, b) => (lines[a] ?? 0) - (lines[b] ?? 0) || a - b,
  );
  const lineAt = (k: number): number => lines[order[k] ?? 0] ?? 0;
  /** The first position in `order` whose line is not below `line`. */
  const lower = (line: number): number => {
    let lo = 0;
    let hi = count;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (lineAt(mid) < line) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  };
  return {
    segments: (line) => order.subarray(lower(line), lower(line + 1)),
    segmentsInLines: (first, last) =>
      order.subarray(lower(Math.min(first, last)), lower(Math.max(first, last) + 1)),
    lastAfter: (line) => {
      // The nearest line at or before `line` with segments.
      const end = lower(line + 1);
      if (end === 0) return -1;
      const start = lower(lineAt(end - 1));
      // Its first run: segments that follow each other directly.
      let k = start;
      while (k + 1 < end && order[k + 1] === (order[k] ?? 0) + 1) k += 1;
      return order[k] ?? -1;
    },
  };
}

/** A range of whole editor lines, `first` to `last` (1-based, both
 *  included). */
export interface LineRange {
  readonly first: number;
  readonly last: number;
}

/**
 * The whole lines a selection covers, from the lines of its start and end
 * (`fromLine` <= `toLine`): a partial line counts as selected, except the
 * line a multi-line selection ends at the very start of (`toAtLineStart`),
 * since nothing of it is selected. A plain cursor gives its own line.
 */
export function selectedLines(fromLine: number, toLine: number, toAtLineStart: boolean): LineRange {
  const last = toLine > fromLine && toAtLineStart ? toLine - 1 : toLine;
  return { first: fromLine, last };
}

/** A diagnostic placed on an editor line, ready for the gutter. */
export interface LineMark {
  /** 1-based editor line. */
  readonly line: number;
  readonly severity: "error" | "warning" | "info";
  readonly message: string;
}

/**
 * Diagnostics that belong to a line the editor has (1 to `lineCount`).
 * Diagnostics for line 0 (not tied to a line) or for lines no longer in
 * the text are left for the diagnostics panel.
 */
export function lineMarks(diagnostics: readonly Diagnostic[], lineCount: number): LineMark[] {
  const severity = (s: number): LineMark["severity"] =>
    s === SEVERITY.Error ? "error" : s === SEVERITY.Warning ? "warning" : "info";
  return diagnostics
    .filter((d) => d.srcFile === 0 && d.srcLine >= 1 && d.srcLine <= lineCount)
    .map((d) => ({ line: d.srcLine, severity: severity(d.severity), message: d.message }));
}

/** The marks whose severity is in `shown`: the gutter follows the
 *  diagnostics panel's filters. */
export function shownMarks(marks: readonly LineMark[], shown: ReadonlySet<LineMark["severity"]>): LineMark[] {
  return marks.filter((m) => shown.has(m.severity));
}

/**
 * Runs `task` after `delayMs` of quiet, and hands on only the newest result:
 * a run that finishes after a newer one started is dropped, so a slow old
 * run never overwrites a newer toolpath.
 */
export function latestOnly<T>(
  task: () => Promise<T>,
  onResult: (result: T) => void,
  onError: (error: unknown) => void,
  delayMs: number,
): { schedule: () => void; runNow: () => Promise<void> } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let generation = 0;
  const runNow = async (): Promise<void> => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    generation += 1;
    const mine = generation;
    try {
      const result = await task();
      if (mine === generation) onResult(result);
    } catch (e) {
      if (mine === generation) onError(e);
    }
  };
  const schedule = (): void => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      void runNow();
    }, delayMs);
  };
  return { schedule, runNow };
}
