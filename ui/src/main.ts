// The UI: a toolbar, the program editor, the toolpath viewport, the
// diagnostics panel, and a status line; statistics open in a dialog. The
// lines selected in the editor (or the cursor's line) are drawn bright
// yellow in the toolpath, and clicking the toolpath
// or a diagnostic jumps to its line. Edits do not re-run the program: they
// mark the results out of date (and turn line sync off) until the user
// refreshes with the button or F5.
//
// No G-code meaning here: the core interprets, the UI shows what it got.

import "./style.css";

import { ask, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

import sample from "../../tests/golden/basic_mill.nc?raw";
import { decodeRenderBuffer, type RenderBuffer } from "./buffer/decode";
import { el } from "./dom";
import { GcodeEditor } from "./editor/editor";
import { baseName, detectLineEnding, fileBytes, openPath, type LineEnding } from "./files";
import { Freshness } from "./freshness";
import { buildLineIndex, latestOnly, lineMarks, shownMarks, type LineIndex, type LineMark, type LineRange } from "./editor/lines";
import { addSplitter, dblclickToggles, Maximiser } from "./layout/layout";
import { layoutToSave, paneName, restoredShare, shownSeverities, type PaneLayout } from "./layout/panes";
import {
  inApp,
  interpret,
  listDialects,
  listMachines,
  loadFile,
  machineFolder,
  rememberChoice,
  rememberLayout,
  rememberedSettings,
  resetLayout,
  saveFile,
  selectMachine,
  setRunOptions,
  type MachineEntry,
} from "./ipc/backend";
import { DiagnosticsPanel } from "./panels/panel";
import { showMachinesDialog } from "./machines-dialog";
import { startChoice } from "./remembered";
import { showStatsDialog } from "./panels/stats-dialog";
import { measurePanel, type Measurement } from "./viewport/measure";
import {
  preferArrivingMove,
  readoutRows,
  restingFor,
  restingPoint,
  type Pinned,
  type ToolPoint,
} from "./viewport/readout";
import { LEGEND, buildToolpath } from "./viewport/toolpath";
import { Viewport } from "./viewport/viewport";
import { VIEWS, type ViewName } from "./viewport/views";

/** The bundled sample's name, and its name when saved. */
const SAMPLE_NAME = "basic_mill.nc (sample)";
const SAMPLE_FILE = "basic_mill.nc";

/** File types offered by the open and save dialogs. */
const FILTERS = [
  { name: "G-code programs", extensions: ["nc", "ngc", "tap", "cnc", "gcode", "iso", "mpf", "txt"] },
  { name: "All files", extensions: ["*"] },
];

/** The program in the editor and the file it came from. */
interface ProgramFile {
  /** Null for the sample, or a program not saved yet. */
  readonly path: string | null;
  readonly name: string;
  readonly ending: LineEnding;
  /** The file had bytes that are not UTF-8 (shown as U+FFFD). */
  readonly notUtf8: boolean;
}

const status = el("span", { className: "status" });
const setStatus = (text: string): void => {
  status.textContent = text;
};

async function main(app: HTMLElement): Promise<void> {
  const machineSelect = el("select", { title: "Machine" });
  const dialectSelect = el("select", { title: "Dialect" });
  const openButton = el("button", { type: "button", textContent: "Open…", title: "Open a program (Ctrl+O)" });
  const saveButton = el("button", { type: "button", textContent: "Save", title: "Save (Ctrl+S)" });
  const saveAsButton = el("button", { type: "button", textContent: "Save As…", title: "Save as (Ctrl+Shift+S)" });
  const machinesButton = el("button", { type: "button", textContent: "Machines…", title: "Where your machine files go" });
  const sampleButton = el("button", { type: "button", textContent: "Sample" });
  const viewButtons = (Object.keys(VIEWS) as ViewName[]).map((name) =>
    el("button", { type: "button", textContent: VIEWS[name].label, className: "view" }),
  );
  const fitButton = el("button", { type: "button", textContent: "Fit" });
  const resetLayoutButton = el("button", {
    type: "button",
    textContent: "Reset layout",
    title: "Default pane sizes and window size",
  });
  const statsButton = el("button", { type: "button", textContent: "Statistics…", disabled: true });
  const measureButton = el("button", {
    type: "button",
    textContent: "Measure",
    title: "Measure mode: drag from one node to another. Click again to leave.",
    className: "toggle",
  });
  const fileLabel = el("span", { className: "file" });
  const editorBox = el("div", { className: "editor" });
  const viewportBox = el("div", { className: "viewport" });
  const panelBox = el("div", { className: "panel" });
  const rightBox = el("div", { className: "right" }, viewportBox, panelBox);
  const mainBox = el("main", {}, editorBox, rightBox);

  app.replaceChildren(
    el(
      "header",
      {},
      el("strong", {}, "GCode EditSim"),
      machineSelect,
      machinesButton,
      dialectSelect,
      openButton,
      saveButton,
      saveAsButton,
      sampleButton,
      fileLabel,
      statsButton,
      measureButton,
      resetLayoutButton,
      // The view buttons wrap to the next row together, kept to the right.
      el("span", { className: "views" }, ...viewButtons, fitButton),
    ),
    mainBox,
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

  let file: ProgramFile = { path: null, name: SAMPLE_NAME, ending: "\n", notUtf8: false };
  /** The editor's text differs from the file (or the sample). */
  let dirty = false;
  const showFileName = (): void => {
    fileLabel.textContent = file.name + (dirty ? " •" : "");
    fileLabel.title = file.path ?? file.name;
    document.title = `${dirty ? "• " : ""}${file.name} – GCode EditSim`;
  };
  showFileName();
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
  // What the colours mean (bottom left of the view); the header folds it.
  const legendToggle = el("button", { type: "button", className: "legend-toggle", textContent: "Colours ▾" });
  const legendList = el(
    "ul",
    {},
    ...LEGEND.map((entry) => {
      const swatch = el("span", { className: `swatch ${entry.style}` });
      swatch.style.setProperty("--swatch", entry.color);
      return el("li", {}, swatch, entry.label);
    }),
  );
  const legend = el("div", { className: "legend" }, legendToggle, legendList);
  const showLegendFolded = (folded: boolean): void => {
    legendList.hidden = folded;
    legendToggle.textContent = folded ? "Colours ▸" : "Colours ▾";
  };
  viewportBox.append(readout, staleBar, legend);
  const panel = new DiagnosticsPanel(panelBox);

  // Resizing: drag the borders between the panes. The shares are null at
  // the default layout; they are remembered when a drag ends (M4.2).
  let editorShare: number | null = null;
  let panelShare: number | null = null;
  const showEditorShare = (share: number | null): void => {
    editorShare = share;
    editorBox.style.flexBasis = share === null ? "" : `${(share * 100).toFixed(2)}%`;
  };
  const showPanelShare = (share: number | null): void => {
    panelShare = share;
    panelBox.style.flexBasis = share === null ? "" : `${(share * 100).toFixed(2)}%`;
  };
  /** False until the remembered layout is restored, and while restoring. */
  let saveLayoutChanges = false;
  const saveLayout = (): void => {
    if (!saveLayoutChanges) return;
    // Losing a layout change is not worth interrupting the user for.
    rememberLayout(
      layoutToSave({
        editor: editorShare,
        panel: panelShare,
        maximised: maximiser.maximised,
        legendFolded: legendList.hidden === true,
        shown: panel.shownSeverities(),
      }),
    ).catch(() => undefined);
  };
  legendToggle.addEventListener("click", () => {
    showLegendFolded(legendList.hidden !== true);
    saveLayout();
  });
  addSplitter(mainBox, rightBox, "x", showEditorShare, saveLayout);
  addSplitter(
    rightBox,
    panelBox,
    "y",
    (share) => {
      showPanelShare(1 - share);
    },
    saveLayout,
  );
  // Maximising: double-click the view or the diagnostics list; the editor
  // has a button instead, so double-click still selects a word there.
  const maximiser = new Maximiser(mainBox);
  maximiser.addExitButton("viewport", viewportBox);
  maximiser.addExitButton("panel", panelBox);
  const editorMax = el("button", { type: "button", className: "editor-max" });
  editorBox.append(editorMax);
  const showEditorMax = (): void => {
    const on = maximiser.maximised === "editor";
    editorMax.textContent = on ? "Exit full screen" : "⤢";
    editorMax.title = on ? "Back to the normal layout" : "Maximise the editor";
    editorMax.classList.toggle("on", on);
  };
  showEditorMax();
  maximiser.onChange = () => {
    showEditorMax();
    saveLayout();
  };
  /** Shows a remembered layout, fitted to the window as it is now. */
  const restoreLayout = (layout: PaneLayout | null): void => {
    showEditorShare(restoredShare(layout?.editor, mainBox.getBoundingClientRect().width));
    showPanelShare(restoredShare(layout?.panel, rightBox.getBoundingClientRect().height));
    maximiser.set(paneName(layout?.maximised));
    showLegendFolded(layout?.legend_folded === true);
    panel.setShown(shownSeverities(layout?.hidden_diagnostics));
  };
  resetLayoutButton.addEventListener("click", () => {
    saveLayoutChanges = false;
    restoreLayout(null);
    saveLayoutChanges = true;
    void resetLayout();
  });
  editorMax.addEventListener("click", () => {
    maximiser.toggle("editor");
  });
  viewportBox.addEventListener("dblclick", (e) => {
    if (dblclickToggles(e.target)) maximiser.toggle("viewport");
  });
  panelBox.addEventListener("dblclick", (e) => {
    const target = e.target as Element | null;
    if (dblclickToggles(target) && target?.closest(".panel-body") != null) maximiser.toggle("panel");
  });

  // The gutter shows only the severities the panel's filters show.
  let allMarks: LineMark[] = [];
  const showMarks = (): void => {
    editor.setMarks(shownMarks(allMarks, panel.shownSeverities()));
  };
  panel.onFilter = () => {
    showMarks();
    saveLayout();
  };
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
  /** Which segments each line produced, for the current buffer. */
  let lineIndex: LineIndex = buildLineIndex(0, () => 0);
  /** Fit the view after the next run (a new program, machine or dialect). */
  let fitNext = true;
  /** A note for the status line after the next run (why the remembered
   *  machine or dialect was not used), shown once. */
  let notice: string | null = null;
  const withNotice = (text: string): string => {
    const shown = notice === null ? text : `${notice} ${text}`;
    notice = null;
    return shown;
  };

  /** The editor cursor's line, and where the tool is after it has run. */
  let cursorLine = 1;
  let resting: ToolPoint | null = null;
  /** The toolpath point under the pointer, if any. */
  let hovered: ToolPoint | null = null;
  /** The last clicked point, where the marker rests while the cursor stays
   *  on the line the click went to. */
  let pinned: Pinned | null = null;

  /** Moves the tool marker and the readout to the hovered point, or else
   *  to where the tool is after the cursor's line. Hovering never moves
   *  the editor's cursor; a click does. */
  /** What the readout shows now, so an unchanged point is not redrawn. */
  let shownReadout = "";
  const showMarker = (): void => {
    // In measure mode the panel shows the measurement instead.
    if (viewport.measureMode) return;
    const rb = current;
    const p = hovered ?? resting;
    viewport.setMarker(p?.point ?? null);
    if (!rb || !p) {
      readout.hidden = true;
      return;
    }
    const where = hovered
      ? "Under the pointer"
      : pinned?.point === p
        ? "Clicked point"
        : `After line ${String(cursorLine)}`;
    const title = where + (freshness.stale ? " (out of date)" : "");
    const key = `${title}|${String(p.segment)}|${p.place}|${p.point.join(",")}`;
    if (key === shownReadout && !readout.hidden) return;
    shownReadout = key;
    readout.replaceChildren(
      el("div", { className: "title", textContent: title }),
      // Program coordinates come with M3.5.
      el("div", { className: "caption", textContent: "Machine coordinates" }),
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
    if (stale) viewport.highlight([]);
    for (const box of [viewportBox, panelBox, editorBox]) box.classList.toggle("stale", stale);
  };

  const showLine = (line: number): void => {
    if (freshness.stale) return;
    cursorLine = line;
    if (!current) return;
    const rb = current;
    if (pinned && pinned.line !== line) pinned = null;
    resting = restingFor(pinned, line, restingPoint(rb, lineIndex.lastAfter(line)));
    showMarker();
  };

  /** The lines selected in the editor (the cursor's line when nothing is
   *  selected), drawn bright yellow in the view (M4.3). */
  let selection: LineRange = { first: 1, last: 1 };
  const showSelection = (quiet = false): void => {
    // Line numbers may have moved since the run: no highlight while out of date.
    if (freshness.stale || !current) {
      viewport.highlight([]);
      return;
    }
    // Every run of each line: a subprogram line is highlighted in each call.
    const segments = lineIndex.segmentsInLines(selection.first, selection.last);
    viewport.highlight(segments);
    if (quiet) return;
    const lines =
      selection.first === selection.last
        ? `Line ${String(selection.first)}`
        : `Lines ${String(selection.first)}–${String(selection.last)}`;
    const n = segments.length;
    setStatus(n === 0 ? `${lines}: no toolpath` : `${lines}: ${String(n)} segment${n === 1 ? "" : "s"}`);
  };
  /** Redraws the selection at most once a frame while it is being dragged. */
  let selectionFrame = 0;
  const scheduleSelection = (): void => {
    if (selectionFrame !== 0) return;
    selectionFrame = requestAnimationFrame(() => {
      selectionFrame = 0;
      showSelection();
    });
  };

  const runner = latestOnly(
    async () => {
      const token = freshness.runStarted();
      const rb = decodeRenderBuffer(await interpret(new TextEncoder().encode(editor.text()), file.name));
      return { rb, token };
    },
    ({ rb, token }) => {
      current = rb;
      lineIndex = buildLineIndex(rb.header.segmentCount, (i) => rb.segmentLine(i));
      freshness.resultShown(token);
      showFreshness();
      statsButton.disabled = false;
      viewport.show(buildToolpath(rb), fitNext);
      fitNext = false;
      allMarks = lineMarks(rb.diagnostics, editor.lineCount());
      showMarks();
      panel.show(rb);
      hovered = null;
      pinned = null;
      showLine(editor.cursorLineNumber());
      selection = editor.selectedLines();
      showSelection(true);
      const d = rb.header.diagCount;
      setStatus(
        withNotice(
          `${String(rb.header.segmentCount)} segments, ${String(d)} diagnostic${d === 1 ? "" : "s"}. Click the toolpath to find its line.`,
        ),
      );
    },
    (e) => {
      setStatus(withNotice(`Could not run the program: ${String(e)}`));
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
    if (!dirty) {
      dirty = true;
      showFileName();
    }
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
      return;
    }
    if (!(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    if (key === "s") {
      e.preventDefault();
      void saveProgram(e.shiftKey);
    } else if (key === "o") {
      e.preventDefault();
      void openProgram();
    }
  });
  editor.onCursorLine = (line) => {
    showLine(line);
  };
  editor.onSelectedLines = (range) => {
    selection = range;
    scheduleSelection();
  };
  viewport.onHover = (p) => {
    hovered = p && current ? preferArrivingMove(current, p) : null;
    showMarker();
  };
  /** Shows the measurement panel in place of the hover readout. */
  const showMeasurement = (m: Measurement | null): void => {
    const panel = measurePanel(m);
    const cells = (values: readonly string[], cls = ""): HTMLElement[] =>
      values.map((v) => el("span", { className: `num ${cls}`, textContent: v }));
    readout.replaceChildren(
      el("div", { className: "title", textContent: "Measure" }),
      el("div", { className: "caption", textContent: "Machine coordinates" }),
      el(
        "div",
        { className: "measure-grid" },
        el("span"),
        ...cells(["X", "Y", "Z"], "head"),
        ...panel.rows.flatMap((r) => [el("span", { className: "label", textContent: r.label }), ...cells(r.values)]),
      ),
      ...(panel.distance === null
        ? []
        : [el("div", { className: "row distance" }, el("span", { textContent: "Distance" }), el("span", { textContent: panel.distance }))]),
      ...(panel.hint === null ? [] : [el("div", { className: "hint", textContent: panel.hint })]),
    );
    readout.hidden = false;
  };
  viewport.onMeasure = (m) => {
    if (viewport.measureMode) showMeasurement(m);
  };
  measureButton.addEventListener("click", () => {
    const on = !viewport.measureMode;
    measureButton.classList.toggle("active", on);
    measureButton.setAttribute("aria-pressed", String(on));
    viewport.setMeasureMode(on);
    if (on) {
      viewport.setMarker(null);
      showMeasurement(null);
    } else {
      shownReadout = "";
      showMarker();
    }
  });
  statsButton.addEventListener("click", () => {
    if (current) showStatsDialog(current, freshness.stale ? `${file.name} (out of date: refresh first)` : file.name);
  });
  viewport.onPick = (p) => {
    if (!current || !p || freshness.stale) return;
    // The marker rests where the user clicked (not at the end of the line)
    // until the cursor moves to another line.
    const line = current.segment(p.segment).srcLine;
    pinned = { line, point: preferArrivingMove(current, p) };
    if (line === cursorLine) {
      resting = pinned.point;
      showMarker();
    }
    // Moving the cursor highlights the line's segments (onCursorLine).
    editor.revealLine(line);
  };

  const dialects = await listDialects();
  for (const d of dialects) {
    dialectSelect.append(el("option", { value: d.id, textContent: d.name }));
  }
  /** Every listed machine file, and those that can be chosen, in the
   *  select's order. */
  let listed: MachineEntry[] = [];
  let usable: MachineEntry[] = [];
  /** Fills the machine list, keeping the current choice if it is still
   *  there. Returns a short summary. */
  const fillMachines = async (): Promise<string> => {
    const chosen = usable[Number(machineSelect.value)]?.source;
    const machines = await listMachines();
    listed = machines;
    usable = machines.filter((m) => m.problem === null);
    machineSelect.replaceChildren(
      ...usable.map((m, i) => el("option", { value: String(i), textContent: m.label })),
      ...machines
        .filter((x) => x.problem !== null)
        .map((m) => el("option", { disabled: true, textContent: `${m.label} (${m.problem ?? ""})` })),
    );
    const keep = usable.findIndex((m) => m.source.kind === chosen?.kind && m.source.id === chosen.id);
    if (keep >= 0) machineSelect.value = String(keep);
    const problems = machines.length - usable.length;
    return `${String(usable.length)} machine file${usable.length === 1 ? "" : "s"} listed${
      problems > 0 ? `, ${String(problems)} with problems (shown greyed out)` : ""
    }.`;
  };
  await fillMachines();
  // Start with the machine and dialect of the last session, if they can
  // still be used.
  const remembered = await rememberedSettings().catch((e: unknown) => ({
    machine: null,
    dialect: null,
    layout: null,
    problem: String(e),
  }));
  restoreLayout(remembered.layout);
  saveLayoutChanges = true;
  const start = startChoice(listed, dialects, remembered);
  if (start.machine >= 0) machineSelect.value = String(start.machine);
  dialectSelect.value = start.dialect;
  notice = start.note;

  const applySettings = async (): Promise<void> => {
    const machine = usable[Number(machineSelect.value)];
    if (machine) await selectMachine(machine.source);
    await setRunOptions(dialectSelect.value, false);
  };
  const rerunFitted = (): void => {
    fitNext = true;
    void runner.runNow();
  };
  /** Remembers the user's choice for the next session. Only the user's
   *  own choices: a fallback at start-up keeps what was remembered, so a
   *  machine file that is broken for a moment is tried again next time. */
  const remember = (): void => {
    const machine = usable[Number(machineSelect.value)];
    if (!machine) return;
    rememberChoice(machine.source, dialectSelect.value).catch((e: unknown) => {
      notice = `Could not remember the machine and dialect: ${String(e)}.`;
    });
  };
  machineSelect.addEventListener("change", () => {
    void applySettings().then(rerunFitted);
    remember();
  });
  dialectSelect.addEventListener("change", () => {
    void applySettings().then(rerunFitted);
    remember();
  });
  machinesButton.addEventListener("click", () => {
    void machineFolder().then((folder) => {
      showMachinesDialog(folder, async () => {
        const before = machineSelect.value;
        const summary = await fillMachines();
        if (machineSelect.value !== before) {
          await applySettings();
          rerunFitted();
        }
        return summary;
      });
    });
  });

  /** Asks before throwing away unsaved edits. True to go ahead. */
  const confirmDiscard = async (): Promise<boolean> =>
    !dirty ||
    ask(`${file.name} has unsaved changes. Discard them?`, {
      title: "Unsaved changes",
      kind: "warning",
      okLabel: "Discard",
      cancelLabel: "Cancel",
    });

  const load = (text: string, next: ProgramFile): void => {
    file = next;
    dirty = false;
    showFileName();
    editor.setText(text);
    rerunFitted();
  };

  const openProgram = async (): Promise<void> => {
    if (!(await confirmDiscard())) return;
    const path = await openDialog({ multiple: false, directory: false, filters: FILTERS });
    if (typeof path !== "string") return;
    const result = await openPath(path, loadFile);
    if (!result.ok) {
      setStatus(`Could not read ${result.path}: ${result.error}`);
      return;
    }
    load(result.text, {
      path,
      name: baseName(path),
      ending: detectLineEnding(result.text),
      notUtf8: result.notUtf8,
    });
    if (result.notUtf8) {
      setStatus(`${baseName(path)} is not UTF-8 text: some characters are shown as \u{FFFD}.`);
    }
  };

  /** Saves to the file's path, or asks for one (always, with `asNew`). */
  const saveProgram = async (asNew: boolean): Promise<void> => {
    let path = file.path;
    if (asNew || path === null) {
      path = await saveDialog({ defaultPath: file.path ?? (file.name === SAMPLE_NAME ? SAMPLE_FILE : file.name), filters: FILTERS });
      if (path === null) return;
    }
    if (
      file.notUtf8 &&
      !(await ask(
        `${file.name} was not UTF-8 text. Characters shown as \u{FFFD} will be saved as \u{FFFD}, changing those bytes. Save anyway?`,
        { title: "Not UTF-8", kind: "warning", okLabel: "Save", cancelLabel: "Cancel" },
      ))
    ) {
      return;
    }
    try {
      await saveFile(path, fileBytes(editor.text(), file.ending));
    } catch (e) {
      setStatus(`Could not save ${path}: ${String(e)}`);
      return;
    }
    file = { ...file, path, name: baseName(path), notUtf8: false };
    dirty = false;
    showFileName();
    setStatus(`Saved ${path}`);
  };

  openButton.addEventListener("click", () => {
    void openProgram();
  });
  saveButton.addEventListener("click", () => {
    void saveProgram(false);
  });
  saveAsButton.addEventListener("click", () => {
    void saveProgram(true);
  });
  sampleButton.addEventListener("click", () => {
    void confirmDiscard().then((go) => {
      if (go) load(sample, { path: null, name: SAMPLE_NAME, ending: "\n", notUtf8: false });
    });
  });
  // Ask before closing the window with unsaved changes.
  void getCurrentWindow().onCloseRequested(async (e) => {
    if (!(await confirmDiscard())) e.preventDefault();
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
