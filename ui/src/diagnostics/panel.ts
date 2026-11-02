// The diagnostics panel (Ctrl+Shift+D, M5.2): a live plain-text report of
// timings, graphics and system facts, with a button that copies it, so the
// owner can paste it into a chat or an issue. It floats in a corner and
// does not block the app, so it can stay open while the problem happens.

import { el } from "../dom";
import { formatReport, type DiagnosticsInfo } from "./report";
import type { Perf } from "./stats";

/** Copies `text`: the Clipboard API, or a selected text area where that is
 *  missing or refused (some WebKitGTK builds). True when it worked. */
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = el("textarea", { value: text });
    area.style.cssText = "position:fixed;left:-9999px;top:0";
    document.body.append(area);
    area.select();
    // Deprecated, but the only fallback when the Clipboard API is refused.
    // eslint-disable-next-line @typescript-eslint/no-deprecated
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}

/** Shows the panel; `gather` collects the facts each time it refreshes.
 *  Returns a function that closes it. */
export function showDiagnostics(gather: () => DiagnosticsInfo, perf: Perf, onClosed: () => void): () => void {
  const report = el("pre", { className: "diagnostics-report" });
  const status = el("span", { className: "note" });
  const copy = el("button", { type: "button", textContent: "Copy" });
  const reset = el("button", { type: "button", textContent: "Reset timings" });
  const close = el("button", { type: "button", textContent: "Close" });
  const dialog = el(
    "dialog",
    { className: "app-dialog diagnostics-dialog" },
    el("h2", { textContent: "Diagnostics" }),
    el("p", {
      className: "note",
      textContent:
        "Timings, graphics and system facts to help find problems. Nothing personal is included. " +
        "Copy it and paste it into the chat or an issue.",
    }),
    report,
    el("div", { className: "buttons" }, status, reset, copy, close),
  );
  const refresh = (): void => {
    report.textContent = formatReport(gather());
  };
  // Frame intervals, measured only while the panel is open.
  let frame = 0;
  let lastFrame = 0;
  const tick = (now: number): void => {
    if (lastFrame !== 0) perf.frame.add(now - lastFrame);
    lastFrame = now;
    frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);
  const timer = window.setInterval(refresh, 500);
  copy.addEventListener("click", () => {
    refresh();
    void copyText(report.textContent).then((ok) => {
      status.textContent = ok ? "Copied." : "Could not copy: select the text and press Ctrl+C.";
    });
  });
  reset.addEventListener("click", () => {
    for (const s of [perf.inputDelay, perf.keyRepeat, perf.keyHandling, perf.follow, perf.render, perf.frame]) s.reset();
    perf.heldBack = 0;
    lastFrame = 0;
    refresh();
  });
  close.addEventListener("click", () => {
    dialog.close();
  });
  dialog.addEventListener("close", () => {
    cancelAnimationFrame(frame);
    window.clearInterval(timer);
    dialog.remove();
    onClosed();
  });
  document.body.append(dialog);
  refresh();
  dialog.show();
  return () => {
    dialog.close();
  };
}
