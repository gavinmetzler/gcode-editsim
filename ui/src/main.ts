// The UI: a toolbar, the program editor, the toolpath viewport, the
// diagnostics panel, and a status line; statistics open in a dialog. The
// cursor's line is highlighted in the toolpath, and clicking the toolpath
// or a diagnostic jumps to its line. Edits do not re-run the program: they
// mark the results out of date (and turn line sync off) until the user
// refreshes with the button or F5.
//
// No G-code meaning here: the core interprets, the UI shows what it got.

import "./style.css";

import sample from "../../tests/golden/basic_mill.nc?raw";
import { decodeRenderBuffer, type RenderBuffer } from "./buffer/decode";
import { el } from "./dom";
import { GcodeEditor } from "./editor/editor";
import { Freshness } from "./freshness";
import { latestOnly, lineMarks, segmentRange } from "./editor/lines";
import {
  inApp,
  interpret,
  listDialects,
  listMachines,
  selectMachine,
  setRunOptions,
  type MachineEntry,
} from "./ipc/backend";
import { readProgramFile } from "./open-file";
import { DiagnosticsPanel } from "./panels/panel";
import { showStatsDialog } from "./panels/stats-dialog";
import { preferArrivingMove, readoutRows, restingPoint, type ToolPoint } from "./viewport/readout";
import { buildToolpath } from "./viewport/toolpath";
import { Viewport } from "./viewport/viewport";
import { VIEWS, type ViewName } from "./viewport/views";

const status = el("span", { className: "status" });
const setStatus = (text: string): void => {
  status.textContent = text;
};

async function main(app: HTMLElement): Promise<void> {
  const machineSelect = el("select", { title: "Machine" });
  const dialectSelect = el("select", { title: "Dialect" });
  const fileInput = el("input", { type: "file" });
  const openButton = el("button", { type: "button", textContent: "Open…" });
  const sampleButton = el("button", { type: "button", textContent: "Sample" });
  const viewButtons = (Object.keys(VIEWS) as ViewName[]).map((name) =>
    el("button", { type: "button", textContent: VIEWS[name].label, className: "view" }),
  );
  const fitButton = el("button", { type: "button", textContent: "Fit" });
  const statsButton = el("button", { type: "button", textContent: "Statistics…", disabled: true });
  const fileLabel = el("span", { className: "file" });
  const editorBox = el("div", { className: "editor" });
  const viewportBox = el("div", { className: "viewport" });
  const panelBox = el("div", { className: "panel" });
  fileInput.hidden = true;

  app.replaceChildren(
    el(
      "header",
      {},
      el("strong", {}, "GCode EditSim"),
      machineSelect,
      dialectSelect,
      openButton,
      sampleButton,
      fileInput,
      fileLabel,
      statsButton,
      el("span", { className: "spacer" }),
      ...viewButtons,
      fitButton,
    ),
    el("main", {}, editorBox, el("div", { className: "right" }, viewportBox, panelBox)),
    el(
      "footer",
      {},
      status,
      el("span", {
        className: "disclaimer",
        textContent: "A visualization aid, not a safety check: verify programs before running them.",
      }),
    ),
  );

  let fileName = "basic_mill.nc (sample)";
  fileLabel.textContent = fileName;
  const editor = new GcodeEditor(editorBox, sample);
  const viewport = new Viewport(viewportBox);
  const readout = el("div", { className: "readout" });
  readout.hidden = true;
  const refreshButton = el("button", { type: "button", textContent: "Refresh (F5)" });
  const staleBar = el(
    "div",
    { className: "stale-bar" },
    el("span", { textContent: "Program edited: the view is out of date." }),
    refreshButton,
  );
  staleBar.hidden = true;
  viewportBox.append(readout, staleBar);
  const panel = new DiagnosticsPanel(panelBox);
  const freshness = new Freshness();
  panel.onLine = (line) => {
    // Line numbers may have moved since the run: no sync while out of date.
    if (!freshness.stale) editor.revealLine(line);
  };
  (Object.keys(VIEWS) as ViewName[]).forEach((name, i) => {
    viewButtons[i]?.addEventListener("click", () => {
      viewport.setView(name);
    });
  });
  fitButton.addEventListener("click", () => {
    viewport.fit();
  });

  if (!inApp()) {
    setStatus("Running outside the app: open the desktop app to interpret programs.");
    panel.clear("Programs are interpreted in the desktop app.");
    return;
  }

  let current: RenderBuffer | null = null;
  /** Fit the view after the next run (a new program, machine or dialect). */
  let fitNext = true;

  /** The editor cursor's line, and where the tool is after it has run. */
  let cursorLine = 1;
  let resting: ToolPoint | null = null;
  /** The toolpath point under the pointer, if any. */
  let hovered: ToolPoint | null = null;

  /** Moves the tool marker and the readout to the hovered point, or else
   *  to where the tool is after the cursor's line. Hovering never moves
   *  the editor's cursor; a click does. */
  const showMarker = (): void => {
    const rb = current;
    const p = hovered ?? resting;
    viewport.setMarker(p?.point ?? null);
    if (!rb || !p) {
      readout.hidden = true;
      return;
    }
    const title =
      (hovered ? "Under the pointer" : `After line ${String(cursorLine)}`) + (freshness.stale ? " (out of date)" : "");
    readout.replaceChildren(
      el("div", { className: "title", textContent: title }),
      ...readoutRows(rb, p).map((r) =>
        el("div", { className: "row" }, el("span", { textContent: r.label }), el("span", { textContent: r.value })),
      ),
    );
    readout.hidden = false;
  };

  /** Marks everything shown as out of date (or current again), and turns
   *  line sync off while it is out of date. */
  const showFreshness = (): void => {
    const stale = freshness.stale && current !== null;
    staleBar.hidden = !stale;
    for (const box of [viewportBox, panelBox, editorBox]) box.classList.toggle("stale", stale);
  };

  const showLine = (line: number, quiet = false): void => {
    if (freshness.stale) return;
    cursorLine = line;
    if (!current) return;
    const rb = current;
    const { start, end } = segmentRange(rb.header.segmentCount, (i) => rb.segmentLine(i), line);
    const segments = Array.from({ length: end - start }, (_, k) => start + k);
    viewport.highlight(segments);
    resting = restingPoint(rb, end - 1);
    showMarker();
    if (quiet) return;
    setStatus(
      segments.length === 0
        ? `Line ${String(line)}: no toolpath`
        : `Line ${String(line)}: ${String(segments.length)} segment${segments.length === 1 ? "" : "s"}`,
    );
  };

  const runner = latestOnly(
    async () => {
      const token = freshness.runStarted();
      const rb = decodeRenderBuffer(await interpret(new TextEncoder().encode(editor.text()), fileName));
      return { rb, token };
    },
    ({ rb, token }) => {
      current = rb;
      freshness.resultShown(token);
      showFreshness();
      statsButton.disabled = false;
      viewport.show(buildToolpath(rb), fitNext);
      fitNext = false;
      editor.setMarks(lineMarks(rb.diagnostics, editor.lineCount()));
      panel.show(rb);
      hovered = null;
      showLine(editor.cursorLineNumber(), true);
      const d = rb.header.diagCount;
      setStatus(
        `${String(rb.header.segmentCount)} segments, ${String(d)} diagnostic${d === 1 ? "" : "s"}. Click the toolpath to find its line.`,
      );
    },
    (e) => {
      setStatus(`Could not run the program: ${String(e)}`);
      panel.clear(`Could not run the program: ${String(e)}`);
      current = null;
      statsButton.disabled = true;
      hovered = null;
      resting = null;
      showMarker();
      showFreshness();
    },
    // Never scheduled: runs start from a refresh or a load, not a timer.
    0,
  );

  const refresh = (): void => {
    void runner.runNow();
  };
  editor.onChange = () => {
    freshness.edited();
    showFreshness();
    showMarker();
  };
  refreshButton.addEventListener("click", refresh);
  window.addEventListener("keydown", (e) => {
    if (e.key === "F5") {
      // Also stops the web view from reloading the page.
      e.preventDefault();
      refresh();
    }
  });
  editor.onCursorLine = (line) => {
    showLine(line);
  };
  viewport.onHover = (p) => {
    hovered = p && current ? preferArrivingMove(current, p) : null;
    showMarker();
  };
  statsButton.addEventListener("click", () => {
    if (current) showStatsDialog(current, freshness.stale ? `${fileName} (out of date: refresh first)` : fileName);
  });
  viewport.onPick = (pick) => {
    if (!current || !pick || freshness.stale) return;
    // Moving the cursor highlights the line's segments (onCursorLine).
    editor.revealLine(current.segment(pick.segment).srcLine);
  };

  const [dialects, machines] = await Promise.all([listDialects(), listMachines()]);
  for (const d of dialects) {
    dialectSelect.append(el("option", { value: d.id, textContent: d.name }));
  }
  const usable = machines.filter((m) => m.problem === null);
  usable.forEach((m: MachineEntry, i) => {
    machineSelect.append(el("option", { value: String(i), textContent: m.label }));
  });
  for (const m of machines.filter((x) => x.problem !== null)) {
    machineSelect.append(el("option", { disabled: true, textContent: `${m.label} (${m.problem ?? ""})` }));
  }

  const applySettings = async (): Promise<void> => {
    const machine = usable[Number(machineSelect.value)];
    if (machine) await selectMachine(machine.source);
    await setRunOptions(dialectSelect.value, false);
  };
  const rerunFitted = (): void => {
    fitNext = true;
    void runner.runNow();
  };
  machineSelect.addEventListener("change", () => {
    void applySettings().then(rerunFitted);
  });
  dialectSelect.addEventListener("change", () => {
    void applySettings().then(rerunFitted);
  });
  const load = (text: string, name: string): void => {
    fileName = name;
    fileLabel.textContent = name;
    editor.setText(text);
    rerunFitted();
  };
  openButton.addEventListener("click", () => {
    fileInput.click();
  });
  fileInput.addEventListener("change", () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    fileInput.value = "";
    void readProgramFile(file).then((result) => {
      if (!result.ok) {
        setStatus(`Could not read ${result.name}: ${result.error}`);
        return;
      }
      load(result.text, result.name);
      if (result.notUtf8) {
        setStatus(`${result.name} is not UTF-8 text: some characters are shown as \u{FFFD}.`);
      }
    });
  });
  sampleButton.addEventListener("click", () => {
    load(sample, "basic_mill.nc (sample)");
  });

  await applySettings();
  await runner.runNow();
}

const app = document.getElementById("app");
if (app) {
  main(app).catch((e: unknown) => {
    setStatus(`Error: ${String(e)}`);
  });
}
