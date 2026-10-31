import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { decodeRenderBuffer, type Diagnostic } from "../src/buffer/decode";
import { latestOnly, lineMarks, segmentRange } from "../src/editor/lines";
import { SEVERITY } from "../src/generated/render_buffer";
import { fixture } from "./fixture";

describe("segmentRange", () => {
  // Fixture segments come from lines 3, 4, 5, 6, 7.
  const rb = decodeRenderBuffer(fixture());
  const range = (line: number) => segmentRange(rb.header.segmentCount, (i) => rb.segmentLine(i), line);

  it("finds a line's segments", () => {
    expect(range(3)).toEqual({ start: 0, end: 1 });
    expect(range(6)).toEqual({ start: 3, end: 4 });
    expect(range(7)).toEqual({ start: 4, end: 5 });
  });

  it("gives an empty range where the line's segments would be", () => {
    expect(range(1)).toEqual({ start: 0, end: 0 });
    expect(range(99)).toEqual({ start: 5, end: 5 });
  });

  it("finds several segments of one line", () => {
    const lines = [1, 2, 2, 2, 5, 5, 9];
    const r = (l: number) => segmentRange(lines.length, (i) => lines[i] ?? 0, l);
    expect(r(2)).toEqual({ start: 1, end: 4 });
    expect(r(5)).toEqual({ start: 4, end: 6 });
    expect(r(3)).toEqual({ start: 4, end: 4 });
    expect(segmentRange(0, () => 0, 1)).toEqual({ start: 0, end: 0 });
  });

  it("never gives an inverted range, even for unordered lines", () => {
    const lines = [5, 1, 3, 2];
    for (let l = 0; l < 7; l += 1) {
      const { start, end } = segmentRange(lines.length, (i) => lines[i] ?? 0, l);
      expect(end).toBeGreaterThanOrEqual(start);
    }
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
