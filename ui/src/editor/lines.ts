// Mapping between editor lines and render-buffer segments and diagnostics.
// Pure functions over numbers, tested without an editor or a GPU.

import type { Diagnostic } from "../buffer/decode";
import { SEVERITY } from "../generated/render_buffer";

/**
 * The segments from source `line`, as a half-open index range. Binary
 * search, like `ir::segments_for_line`: it relies on segments being in
 * program order, which holds while programs have no subprograms or loops
 * (ARCHITECTURE.md section 6.6). An empty range starts where the line's
 * segments would be.
 */
export function segmentRange(
  count: number,
  lineOf: (index: number) => number,
  line: number,
): { readonly start: number; readonly end: number } {
  const lower = (pred: (l: number) => boolean): number => {
    let lo = 0;
    let hi = count;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (pred(lineOf(mid))) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  };
  const start = lower((l) => l < line);
  const end = lower((l) => l <= line);
  return { start, end: Math.max(start, end) };
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
