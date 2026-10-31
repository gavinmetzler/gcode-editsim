// The info panel under the viewport: a Diagnostics tab (click a row to go
// to its line) and a Statistics tab. It shows what the pure modules
// diagnostics.ts and stats.ts produced.

import type { RenderBuffer } from "../buffer/decode";
import { countsText, diagnosticRows, severityCounts, type DiagnosticRow, type SeverityName } from "./diagnostics";
import { computeStats, statSections } from "./stats";

/** Rows drawn at most, so a program with a diagnostic on every line stays
 *  responsive; the rest are counted. */
const MAX_ROWS = 2000;

const SEVERITIES: readonly { readonly name: SeverityName; readonly label: string }[] = [
  { name: "error", label: "Errors" },
  { name: "warning", label: "Warnings" },
  { name: "info", label: "Notes" },
];

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  Object.assign(e, props);
  e.append(...children);
  return e;
}

/** The diagnostics and statistics panel inside `container`. */
export class InfoPanel {
  private readonly diagTab = el("button", { type: "button", className: "tab" });
  private readonly statsTab = el("button", { type: "button", className: "tab", textContent: "Statistics" });
  private readonly filters = new Map<SeverityName, HTMLInputElement>();
  private readonly filterBox = el("span", { className: "filters" });
  private readonly diagBody = el("div", { className: "panel-body diagnostics" });
  private readonly statsBody = el("div", { className: "panel-body statistics" });
  private rows: DiagnosticRow[] = [];

  /** Called with the line of a clicked diagnostic. */
  onLine: (line: number) => void = () => undefined;

  constructor(container: HTMLElement) {
    for (const s of SEVERITIES) {
      const box = el("input", { type: "checkbox", checked: true });
      box.addEventListener("change", () => {
        this.drawDiagnostics();
      });
      this.filters.set(s.name, box);
      this.filterBox.append(el("label", { className: s.name }, box, s.label));
    }
    this.diagTab.addEventListener("click", () => {
      this.select("diagnostics");
    });
    this.statsTab.addEventListener("click", () => {
      this.select("statistics");
    });
    this.diagBody.addEventListener("click", (e) => {
      const row = (e.target as Element | null)?.closest<HTMLElement>("[data-line]");
      const line = Number(row?.dataset.line);
      if (Number.isInteger(line) && line >= 1) this.onLine(line);
    });
    container.replaceChildren(
      el("div", { className: "panel-tabs" }, this.diagTab, this.statsTab, el("span", { className: "spacer" }), this.filterBox),
      this.diagBody,
      this.statsBody,
    );
    this.select("diagnostics");
    this.clear();
  }

  /** Shows the diagnostics and statistics of `rb`. */
  show(rb: RenderBuffer): void {
    this.rows = diagnosticRows(rb.diagnostics);
    this.drawDiagnostics();
    this.drawStats(rb);
  }

  /** Shows a message instead of results (no run yet, or it failed). */
  clear(message = "Nothing run yet."): void {
    this.rows = [];
    this.diagTab.textContent = "Diagnostics";
    this.diagBody.replaceChildren(el("p", { className: "empty", textContent: message }));
    this.statsBody.replaceChildren(el("p", { className: "empty", textContent: message }));
  }

  private select(tab: "diagnostics" | "statistics"): void {
    const diag = tab === "diagnostics";
    this.diagTab.classList.toggle("selected", diag);
    this.statsTab.classList.toggle("selected", !diag);
    this.diagBody.hidden = !diag;
    this.statsBody.hidden = diag;
    this.filterBox.hidden = !diag;
  }

  private drawDiagnostics(): void {
    const counts = severityCounts(this.rows);
    this.diagTab.textContent = `Diagnostics (${countsText(counts)})`;
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

  private drawStats(rb: RenderBuffer): void {
    const sections = statSections(computeStats(rb));
    const parts: Node[] = [];
    for (const section of sections) {
      const table = el("table");
      for (const row of section.rows) {
        table.append(el("tr", {}, el("th", { textContent: row.label }), el("td", { textContent: row.value })));
      }
      parts.push(el("section", {}, el("h3", { textContent: section.title }), table));
    }
    parts.push(
      el("p", {
        className: "empty",
        textContent: "Lengths are as drawn: rapids as straight lines, arcs along their chords. No time estimate yet.",
      }),
    );
    this.statsBody.replaceChildren(...parts);
  }
}
