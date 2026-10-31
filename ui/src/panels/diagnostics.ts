// The diagnostics list: every diagnostic of a run, in line order, with
// counts by severity. Pure data, no DOM, so it is tested without a browser.
// The messages are the core's own; the UI only orders and labels them.

import type { Diagnostic } from "../buffer/decode";
import { DIAG_CODE, SEVERITY } from "../generated/render_buffer";

/** A severity as the UI names it. */
export type SeverityName = "error" | "warning" | "info";

/** The UI name of a `SEVERITY` value. Unknown values count as errors, so
 *  they are never hidden among notes. */
export function severityName(severity: number): SeverityName {
  if (severity === SEVERITY.Info) return "info";
  if (severity === SEVERITY.Warning) return "warning";
  return "error";
}

/** The name of a `DIAG_CODE` value, or its number if it is unknown. */
export function codeName(code: number): string {
  for (const [name, value] of Object.entries(DIAG_CODE)) {
    if (value === code) return name;
  }
  return `Code ${String(code)}`;
}

/** One row of the diagnostics list. */
export interface DiagnosticRow {
  /** 1-based line to jump to, or null for the whole program. */
  readonly line: number | null;
  readonly severity: SeverityName;
  readonly code: string;
  readonly message: string;
}

/** Number of diagnostics of each severity. */
export type SeverityCounts = Readonly<Record<SeverityName, number>>;

/**
 * The rows to list: whole-program diagnostics first, then by line, keeping
 * the core's order within a line. Diagnostics of other files (subprograms,
 * later) have no line in this editor, so they are listed with the whole
 * program for now.
 */
export function diagnosticRows(diagnostics: readonly Diagnostic[]): DiagnosticRow[] {
  return diagnostics
    .map((d, index) => ({ d, index }))
    .map(({ d, index }) => ({
      index,
      row: {
        line: d.srcFile === 0 && d.srcLine >= 1 ? d.srcLine : null,
        severity: severityName(d.severity),
        code: codeName(d.code),
        message: d.message,
      },
    }))
    .sort((a, b) => (a.row.line ?? 0) - (b.row.line ?? 0) || a.index - b.index)
    .map(({ row }) => row);
}

/** Counts `rows` by severity. */
export function severityCounts(rows: readonly DiagnosticRow[]): SeverityCounts {
  const counts = { error: 0, warning: 0, info: 0 };
  for (const r of rows) counts[r.severity] += 1;
  return counts;
}

/** A short summary such as "2 errors, 1 warning", or "No diagnostics". */
export function countsText(counts: SeverityCounts): string {
  const part = (n: number, word: string): string | null =>
    n === 0 ? null : `${String(n)} ${word}${n === 1 ? "" : "s"}`;
  const parts = [part(counts.error, "error"), part(counts.warning, "warning"), part(counts.info, "note")].filter(
    (p) => p !== null,
  );
  return parts.length === 0 ? "No diagnostics" : parts.join(", ");
}
