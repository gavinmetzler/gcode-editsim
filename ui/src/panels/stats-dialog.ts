// The statistics dialog: worked out only when the user asks (Statistics…
// in the toolbar), because a very large program takes a while. Shows every
// section at once, side by side, and closes with OK or Esc.

import type { RenderBuffer } from "../buffer/decode";
import { el } from "../dom";
import { computeStats, statSections } from "./stats";

/** Opens a modal dialog with the statistics of `rb`. */
export function showStatsDialog(rb: RenderBuffer, title: string): void {
  const body = el("div", { className: "stats-body" }, el("p", { className: "empty", textContent: "Working out…" }));
  const ok = el("button", { type: "button", textContent: "OK", autofocus: true });
  const dialog = el(
    "dialog",
    { className: "stats-dialog" },
    el("h2", { textContent: `Statistics: ${title}` }),
    body,
    el(
      "p",
      {
        className: "note",
        textContent:
          "Times are a simple estimate: each move at its feed rate (capped at the machine's maximum feed; feed per revolution times the spindle speed) or at its axes' rapid rates, with no acceleration or deceleration, plus dwells and the machine file's tool change time. Real run times are longer. Lengths are as drawn: rapids as straight lines, arcs along their chords. A visualization aid, not a safety check.",
      },
    ),
    el("div", { className: "buttons" }, ok),
  );
  ok.addEventListener("click", () => {
    dialog.close();
  });
  dialog.addEventListener("close", () => {
    dialog.remove();
  });
  document.body.append(dialog);
  dialog.showModal();

  // Let the dialog paint first, so a long computation shows "Working out…".
  requestAnimationFrame(() => {
    setTimeout(() => {
      if (!dialog.open) return;
      const sections = statSections(computeStats(rb)).map((section) => {
        const table = el("table");
        for (const row of section.rows) {
          table.append(el("tr", {}, el("th", { textContent: row.label }), el("td", { textContent: row.value })));
        }
        return el("section", {}, el("h3", { textContent: section.title }), table);
      });
      body.replaceChildren(...sections);
    }, 0);
  });
}
