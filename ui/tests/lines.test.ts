import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { decodeRenderBuffer, type Diagnostic } from "../src/buffer/decode";
import { buildLineIndex, latestOnly, lineMarks, type LineIndex } from "../src/editor/lines";
import { SEVERITY } from "../src/generated/render_buffer";
import { fixture } from "./fixture";

describe("buildLineIndex", () => {
  // Fixture segments come from lines 3, 4, 5, 6, 7.
  const rb = decodeRenderBuffer(fixture());
  const fixtureIndex = buildLineIndex(rb.header.segmentCount, (i) => rb.segmentLine(i));
  const of = (lines: number[]) => buildLineIndex(lines.length, (i) => lines[i] ?? 0);
  const segs = (index: LineIndex, line: number) => Array.from(index.segments(line));

  it("finds a line's segments in a straight-line program", () => {
    expect(segs(fixtureIndex, 3)).toEqual([0]);
    expect(segs(fixtureIndex, 7)).toEqual([4]);
    expect(segs(fixtureIndex, 2)).toEqual([]);
    expect(segs(of([1, 2, 2, 2, 5, 5]), 2)).toEqual([1, 2, 3]);
    expect(segs(of([]), 1)).toEqual([]);
  });

  it("finds every run of a subprogram line", () => {
    // Main lines 1, 2, then a subprogram (lines 6, 7) run twice, then 3.
    const index = of([1, 2, 6, 7, 6, 7, 3]);
    expect(segs(index, 6)).toEqual([2, 4]);
    expect(segs(index, 7)).toEqual([3, 5]);
    expect(segs(index, 3)).toEqual([6]);
  });

  it("says where the tool is after a line", () => {
    const straight = of([1, 2, 2, 2, 5, 5]);
    expect(straight.lastAfter(2)).toBe(3);
    // No segments on lines 3 and 4: the tool is where line 2 left it.
    expect(straight.lastAfter(4)).toBe(3);
    expect(straight.lastAfter(9)).toBe(5);
    expect(straight.lastAfter(0)).toBe(-1);
  });

  it("uses a subprogram line's first run", () => {
    // Line 6 has two segments per run (2-3, then 5-6).
    const index = of([1, 2, 6, 6, 7, 6, 6, 7]);
    expect(index.lastAfter(6)).toBe(3);
    expect(index.lastAfter(7)).toBe(4);
  });
});

describe("lineMarks", () => {
  const diag = (srcLine: number, severity: number, srcFile = 0): Diagnostic => ({
    srcLine,
    srcFile,
    severity,
    code: 1,
    message: `on ${String(srcLine)}`,
  });

  it("keeps diagnostics on lines the editor has, with their severity", () => {
    const marks = lineMarks(
      [diag(1, SEVERITY.Error), diag(2, SEVERITY.Warning), diag(3, SEVERITY.Info)],
      3,
    );
    expect(marks.map((m) => [m.line, m.severity])).toEqual([
      [1, "error"],
      [2, "warning"],
      [3, "info"],
    ]);
  });

  it("leaves out line 0, lines past the end and other files", () => {
    expect(lineMarks([diag(0, SEVERITY.Error), diag(4, SEVERITY.Error), diag(1, SEVERITY.Error, 1)], 3)).toEqual([]);
  });
});

describe("latestOnly", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("runs once after the edits stop", async () => {
    const task = vi.fn(() => Promise.resolve(1));
    const onResult = vi.fn();
    const r = latestOnly(task, onResult, () => undefined, 300);
    r.schedule();
    await vi.advanceTimersByTimeAsync(200);
    r.schedule();
    await vi.advanceTimersByTimeAsync(200);
    expect(task).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(150);
    expect(task).toHaveBeenCalledTimes(1);
    expect(onResult).toHaveBeenCalledWith(1);
  });

  it("drops a slow older result that finishes after a newer one", async () => {
    const resolvers: ((v: string) => void)[] = [];
    const task = () =>
      new Promise<string>((resolve) => {
        resolvers.push(resolve);
      });
    const results: string[] = [];
    const r = latestOnly(task, (v) => results.push(v), () => undefined, 300);
    const first = r.runNow();
    const second = r.runNow();
    resolvers[1]?.("new");
    resolvers[0]?.("old");
    await Promise.all([first, second]);
    expect(results).toEqual(["new"]);
  });

  it("reports only the newest error", async () => {
    const errors: unknown[] = [];
    let n = 0;
    const r = latestOnly(
      () => Promise.reject(new Error(`run ${String((n += 1))}`)),
      () => undefined,
      (e) => errors.push(e),
      300,
    );
    await r.runNow();
    expect(errors).toHaveLength(1);
  });
});
