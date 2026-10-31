import { describe, expect, it } from "vitest";

import { decodeRenderBuffer, type Diagnostic } from "../src/buffer/decode";
import { DIAG_CODE, SEVERITY } from "../src/generated/render_buffer";
import { codeName, countsText, diagnosticRows, severityCounts, severityName } from "../src/panels/diagnostics";
import { fixture } from "./fixture";

const diag = (srcLine: number, severity: number, code: number, message: string, srcFile = 0): Diagnostic => ({
  srcLine,
  srcFile,
  severity,
  code,
  message,
});

describe("diagnosticRows", () => {
  it("puts whole-program diagnostics first, then lines in order, keeping order within a line", () => {
    const rows = diagnosticRows([
      diag(7, SEVERITY.Error, DIAG_CODE.InvalidArc, "b"),
      diag(3, SEVERITY.Warning, DIAG_CODE.MissingFeedRate, "a"),
      diag(0, SEVERITY.Info, DIAG_CODE.NotYetSimulated, "whole"),
      diag(7, SEVERITY.Info, DIAG_CODE.ControllerDependent, "c"),
      diag(5, SEVERITY.Error, DIAG_CODE.SyntaxError, "other file", 1),
    ]);
    expect(rows.map((r) => [r.line, r.message])).toEqual([
      [null, "whole"],
      [null, "other file"],
      [3, "a"],
      [7, "b"],
      [7, "c"],
    ]);
    expect(rows[2]).toEqual({ line: 3, severity: "warning", code: "MissingFeedRate", message: "a" });
  });

  it("reads the shared fixture's diagnostics", () => {
    const rows = diagnosticRows(decodeRenderBuffer(fixture()).diagnostics);
    expect(rows).toHaveLength(3);
    expect(rows[0]?.line).toBeNull();
  });
});

describe("names and counts", () => {
  it("names severities and codes, unknown severities as errors", () => {
    expect(severityName(SEVERITY.Info)).toBe("info");
    expect(severityName(SEVERITY.Warning)).toBe("warning");
    expect(severityName(SEVERITY.Error)).toBe("error");
    expect(severityName(9)).toBe("error");
    for (const [name, value] of Object.entries(DIAG_CODE)) expect(codeName(value)).toBe(name);
    expect(codeName(999)).toBe("Code 999");
  });

  it("counts by severity and summarises", () => {
    const rows = diagnosticRows([
      diag(1, SEVERITY.Error, 1, ""),
      diag(2, SEVERITY.Error, 1, ""),
      diag(3, SEVERITY.Info, 1, ""),
    ]);
    const counts = severityCounts(rows);
    expect(counts).toEqual({ error: 2, warning: 0, info: 1 });
    expect(countsText(counts)).toBe("2 errors, 1 note");
    expect(countsText({ error: 0, warning: 1, info: 0 })).toBe("1 warning");
    expect(countsText({ error: 0, warning: 0, info: 0 })).toBe("No diagnostics");
  });
});
