// The diagnostics panel under the viewport: every diagnostic of the last
// run (click a row to go to its line), with severity filters. It shows
// what the pure module diagnostics.ts produced. Statistics are not here:
// they are worked out on demand, in a dialog (stats-dialog.ts).

import type { RenderBuffer } from "../buffer/decode";
import { el } from "../dom";
import { countsText, diagnosticRows, severityCounts, type DiagnosticRow, type SeverityName } from "./diagnostics";

/** Rows drawn at most, so a program with a diagnostic on every line stays
 *  responsive; the rest are counted. */
const MAX_ROWS = 2000;

const SEVERITIES: readonly { readonly name: SeverityName; readonly label: string }[] = [
  { name: "error", label: "Errors" },
  { name: "warning", label: "Warnings" },
  { name: "info", label: "Notes" },
];

/** The diagnostics panel inside `container`. */
export class DiagnosticsPanel {
  private readonly title = el("span", { className: "title" });
  private readonly filters = new Map<SeverityName, HTMLInputElement>();
  private readonly filterBox = el("span", { className: "filters" });
  private readonly diagBody = el("div", { className: "panel-body diagnostics" });
  private rows: DiagnosticRow[] = [];

  /** Called with the line of a clicked diagnostic. */
  onLine: (line: number) => void = () => undefined;

  /** Called with the severities shown after a filter changes, so the
   *  editor's gutter can follow. */
  onFilter: (shown: ReadonlySet<SeverityName>) => void = () => undefined;

  constructor(container: HTMLElement) {
    for (const s of SEVERITIES) {
      const box = el("input", { type: "checkbox", checked: true });
      box.addEventListener("change", () => {
        this.drawDiagnostics();
        this.onFilter(this.shownSeverities());
      });
      this.filters.set(s.name, box);
      this.filterBox.append(el("label", { className: s.name }, box, s.label));
    }
    this.diagBody.addEventListener("click", (e) => {
      const row = (e.target as Element | null)?.closest<HTMLElement>("[data-line]");
      const line = Number(row?.dataset.line);
      if (Number.isInteger(line) && line >= 1) this.onLine(line);
    });
    container.replaceChildren(
      el("div", { className: "panel-tabs" }, this.title, el("span", { className: "spacer" }), this.filterBox),
      this.diagBody,
    );
    this.clear();
  }

  /** The severities whose filter box is ticked. */
  shownSeverities(): Set<SeverityName> {
    return new Set(SEVERITIES.map((s) => s.name).filter((n) => this.filters.get(n)?.checked !== false));
  }

  /** Ticks the filter boxes of `shown` and unticks the rest, as if the
   *  user had (redraws, and calls `onFilter`). */
  setShown(shown: ReadonlySet<SeverityName>): void {
    for (const [name, box] of this.filters) box.checked = shown.has(name);
    this.drawDiagnostics();
    this.onFilter(this.shownSeverities());
  }

  /** Shows the diagnostics of `rb`. */
  show(rb: RenderBuffer): void {
    this.rows = diagnosticRows(rb.diagnostics);
    this.drawDiagnostics();
  }

  /** Shows a message instead of results (no run yet, or it failed). */
  clear(message = "Nothing run yet."): void {
    this.rows = [];
    this.title.textContent = "Diagnostics";
    this.diagBody.replaceChildren(el("p", { className: "empty", textContent: message }));
  }

  private drawDiagnostics(): void {
    const counts = severityCounts(this.rows);
    this.title.textContent = `Diagnostics (${countsText(counts)})`;
    const shown = this.rows.filter((r) => this.filters.get(r.severity)?.checked !== false);
    if (shown.length === 0) {
      const text = this.rows.length === 0 ? "No diagnostics." : "All diagnostics are filtered out.";
      this.diagBody.replaceChildren(el("p", { className: "empty", textContent: text }));
      return;
    }
    const table = el("table");
    for (const r of shown.slice(0, MAX_ROWS)) {
      const tr = el(
        "tr",
        { className: r.severity, title: r.line === null ? "Whole program" : `Go to line ${String(r.line)}` },
        el("td", { className: "severity", textContent: r.severity }),
        el("td", { className: "line", textContent: r.line === null ? "–" : String(r.line) }),
        el("td", { className: "code", textContent: r.code }),
        el("td", { className: "message", textContent: r.message }),
      );
      if (r.line !== null) tr.dataset.line = String(r.line);
      table.append(tr);
    }
    const more = shown.length - MAX_ROWS;
    this.diagBody.replaceChildren(
      table,
      ...(more > 0 ? [el("p", { className: "empty", textContent: `…and ${String(more)} more.` })] : []),
    );
  }
}
