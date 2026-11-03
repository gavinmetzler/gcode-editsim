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
import { FIND_TOOLTIP } from "./editor/find";
import { baseName, detectLineEnding, fileBytes, openPath, type LineEnding } from "./files";
import { Freshness } from "./freshness";
import { buildLineIndex, latestOnly, lineMarks, shownMarks, type LineIndex, type LineMark, type LineRange } from "./editor/lines";
import { addSplitter, dblclickToggles, Maximiser } from "./layout/layout";
import {
  colouringFrom,
  layoutToSave,
  paneName,
  restoredShare,
  shownSeverities,
  type Colouring,
  type PaneLayout,
} from "./layout/panes";
import {
  inApp,
  interpret,
  listDialects,
  listMachines,
  loadFile,
  machineFolder,
  programPoint,
  rememberChoice,
  rememberLayout,
  rememberedSettings,
  resetLayout,
  saveFile,
  selectMachine,
  startupFile,
  setRunOptions,
  systemInfo,
  transformProgram,
  type AppSystemInfo,
  type MachineEntry,
} from "./ipc/backend";
import { DiagnosticsPanel } from "./panels/panel";
import { showDiagnostics, type Switch } from "./diagnostics/panel";
import type { DiagnosticsInfo } from "./diagnostics/report";
import { newPerf } from "./diagnostics/stats";
import { KeyHold, behind } from "./catch-up";
import { showMachinesDialog } from "./machines-dialog";
import { machineDialect, startChoice } from "./remembered";
import { showTransformDialog } from "./transform/dialog";
import { decodeEdits, editorChanges } from "./transform/edits";
import { buildRequest } from "./transform/request";
import { showStatsDialog } from "./panels/stats-dialog";
import { ToolsMenu } from "./panels/tools-menu";
import { keepHidden, toolsInOrder } from "./viewport/tool-filter";
import { measurePanel, type Measurement } from "./viewport/measure";
import {
  preferArrivingMove,
  readoutRows,
  restingFor,
  restingPoint,
  type Pinned,
  type ToolPoint,
} from "./viewport/readout";
import { legendEntries } from "./viewport/tool-colours";
import { buildToolpath } from "./viewport/toolpath";
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
  const findButton = el("button", { type: "button", textContent: "Find…", title: FIND_TOOLTIP });
  /** The tools hidden in the view (M5.4): not remembered, and all shown
   *  again for another file (owner, M5_PLAN §6). */
  let hiddenTools: ReadonlySet<number> = new Set();
  const toolsMenu = new ToolsMenu((hidden) => {
    hiddenTools = hidden;
    viewport.setHiddenTools(hidden);
  });
  const statsButton = el("button", { type: "button", textContent: "Statistics…", disabled: true });
  const transformButton = el("button", {
    type: "button",
    textContent: "Transform…",
    title: "Translate, rotate or scale the selected lines (Ctrl+T)",
  });
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
      findButton,
      toolsMenu.button,
      statsButton,
      transformButton,
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
  const legendList = el("ul", {});
  // Colour by move type (as before) or by tool (M5.5), remembered.
  const byMoveButton = el("button", { type: "button", className: "toggle", textContent: "By move" });
  const byToolButton = el("button", { type: "button", className: "toggle", textContent: "By tool" });
  const legendBody = el("div", {}, el("div", { className: "colour-by" }, byMoveButton, byToolButton), legendList);
  const legend = el("div", { className: "legend" }, legendToggle, legendBody);
  let colouring: Colouring = "move";
  /** The program's tools in order of first use, for the colour key. */
  let legendTools: number[] = [];
  const showLegend = (): void => {
    byMoveButton.classList.toggle("active", colouring === "move");
    byToolButton.classList.toggle("active", colouring === "tool");
    legendList.replaceChildren(
      ...legendEntries(colouring, legendTools).map((entry) => {
        const swatch = el("span", { className: `swatch ${entry.style}` });
        swatch.style.setProperty("--swatch", entry.color);
        return el("li", {}, swatch, entry.label);
      }),
    );
  };
  showLegend();
  const showLegendFolded = (folded: boolean): void => {
    legendBody.hidden = folded;
    legendToggle.textContent = folded ? "Colours ▸" : "Colours ▾";
  };
  // Hide or show every rapid (owner's addition to M5.5): a toggle in the
  // view, not remembered, so the app always starts showing them.
  const rapidsButton = el("button", {
    type: "button",
    className: "rapids-toggle toggle",
    textContent: "Hide rapids",
    title: "Hide or show rapid moves. Hidden rapids cannot be clicked or measured to.",
  });
  rapidsButton.addEventListener("click", () => {
    const hide = !rapidsButton.classList.contains("active");
    rapidsButton.classList.toggle("active", hide);
    rapidsButton.textContent = hide ? "Rapids hidden" : "Hide rapids";
    viewport.setRapidsShown(!hide);
  });
  viewportBox.append(readout, staleBar, legend, rapidsButton);
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
        legendFolded: legendBody.hidden === true,
        shown: panel.shownSeverities(),
        colouring,
      }),
    ).catch(() => undefined);
  };
  const setColouring = (next: Colouring): void => {
    colouring = next;
    viewport.setColouring(next);
    showLegend();
  };
  for (const [button, value] of [
    [byMoveButton, "move"],
    [byToolButton, "tool"],
  ] as const) {
    button.addEventListener("click", () => {
      setColouring(value);
      saveLayout();
    });
  }
  legendToggle.addEventListener("click", () => {
    showLegendFolded(legendBody.hidden !== true);
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
    setColouring(colouringFrom(layout));
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
  /** Timings for the diagnostics panel (M5.2). */
  const perf = newPerf();
  /** The last 3D draw's time, for the catch-up rule. */
  let lastDrawMs = 0;
  viewport.onRender = (ms) => {
    lastDrawMs = ms;
    perf.render.add(ms);
  };
  /** How long the last run's steps took, and its render buffer's size. */
  let lastRun: (NonNullable<DiagnosticsInfo["lastRun"]> & { bytes: number }) | null = null;
  /** Which segments each line produced, for the current buffer. */
  let lineIndex: LineIndex = buildLineIndex(0, () => 0);
  /** Fit the view after the next run (a new program, machine or dialect). */
  let fitNext = true;
  /** While the Transform dialog waits for a centre, the next pick goes
   *  here instead (M4.9). */
  let pickForTransform: ((p: ToolPoint | null) => void) | null = null;
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
    if (stale) viewport.highlightLines(null);
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
      viewport.highlightLines(null);
      return;
    }
    // Every run of each line: a subprogram line is highlighted in each call.
    // Both are cheap whatever the size: a view of the line index, and two
    // numbers for the shader (M5.2).
    const segments = lineIndex.segmentsInLines(selection.first, selection.last);
    viewport.highlightLines(selection);
    // Keep the "click a node" hint while the Transform dialog waits.
    if (quiet || pickForTransform) return;
    const lines =
      selection.first === selection.last
        ? `Line ${String(selection.first)}`
        : `Lines ${String(selection.first)}–${String(selection.last)}`;
    const n = segments.length;
    setStatus(n === 0 ? `${lines}: no toolpath` : `${lines}: ${String(n)} segment${n === 1 ? "" : "s"}`);
  };
  /** Follows the editor's cursor and selection in the view, the marker,
   *  the readout and the status, once per frame however fast the cursor
   *  moves (a held Page Up/Down, a drag). Each update is cheap: the
   *  highlight is two numbers for a shader (M5.2). But when key presses
   *  arrive late and the 3D draws are expensive (software rendering), the
   *  highlight and its redraw wait until they stop, so the editor catches
   *  up first; the marker and readout still follow. */
  let followFrame = 0;
  let lastKeyLag = 0;
  let lastKeyAt = Number.NEGATIVE_INFINITY;
  const follow = (): void => {
    followFrame = 0;
    const started = performance.now();
    showLine(editor.cursorLineNumber());
    if (behind(lastKeyLag, started - lastKeyAt, lastDrawMs)) {
      perf.heldBack += 1;
      followFrame = requestAnimationFrame(follow);
      return;
    }
    selection = editor.selectedLines();
    showSelection();
    perf.follow.add(performance.now() - started);
  };
  const followCursor = (): void => {
    if (followFrame === 0) followFrame = requestAnimationFrame(follow);
  };
  // Key timings for the catch-up rule and the diagnostics panel: how long
  // each press waited before the page got it, how fast the keyboard
  // repeats (its own timestamps), and how long the page spends on a press
  // (from this capture listener, before the editor, to the bubbling one,
  // after it).
  const keyHold = new KeyHold();
  let keyStart = 0;
  editorBox.addEventListener(
    "keydown",
    (e) => {
      const now = performance.now();
      keyStart = now;
      if (e.repeat) perf.keyArrival.add(now - lastKeyAt);
      lastKeyLag = Math.max(0, now - e.timeStamp);
      lastKeyAt = now;
      perf.inputDelay.add(lastKeyLag);
      const interval = keyHold.keyDown(e.timeStamp, now, e.repeat);
      if (interval !== null) perf.keyRepeat.add(interval);
    },
    { capture: true },
  );
  editorBox.addEventListener("keydown", () => {
    perf.keyHandling.add(performance.now() - keyStart);
  });
  editorBox.addEventListener(
    "keyup",
    () => {
      keyHold.keyUp();
    },
    { capture: true },
  );

  const runner = latestOnly(
    async () => {
      const token = freshness.runStarted();
      const started = performance.now();
      const bytes = await interpret(new TextEncoder().encode(editor.text()), file.name);
      const rb = decodeRenderBuffer(bytes);
      return { rb, token, interpretMs: performance.now() - started, bytes: bytes.byteLength };
    },
    ({ rb, token, interpretMs, bytes }) => {
      current = rb;
      // This result shows the cursor's line and selection itself.
      cancelAnimationFrame(followFrame);
      followFrame = 0;
      lineIndex = buildLineIndex(rb.header.segmentCount, (i) => rb.segmentLine(i));
      freshness.resultShown(token);
      showFreshness();
      statsButton.disabled = false;
      const building = performance.now();
      const toolpath = buildToolpath(rb);
      const tools = toolsInOrder(toolpath.segmentTool);
      hiddenTools = keepHidden(hiddenTools, tools);
      toolsMenu.setTools(tools, hiddenTools);
      legendTools = tools;
      showLegend();
      const showing = performance.now();
      viewport.show(toolpath, fitNext, hiddenTools);
      lastRun = { interpretMs, buildMs: showing - building, showMs: performance.now() - showing, bytes };
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
    // Line numbers may move: notes from a transformation no longer apply.
    panel.setNotes([]);
    if (!dirty) {
      dirty = true;
      showFileName();
    }
    freshness.edited();
    showFreshness();
    showMarker();
  };
  refreshButton.addEventListener("click", refresh);
  findButton.addEventListener("click", () => {
    editor.openFind();
  });
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
    } else if (key === "t") {
      e.preventDefault();
      openTransform();
    } else if (key === "d" && e.shiftKey) {
      e.preventDefault();
      toggleDiagnostics();
    } else if ((key === "f" || key === "h") && !e.shiftKey && !e.defaultPrevented) {
      // Find and Replace from anywhere in the window (M5.3); the editor
      // handles these keys itself when it has the focus.
      e.preventDefault();
      editor.openFind(key === "h");
    }
  });
  /** The diagnostics panel (Ctrl+Shift+D, M5.2), and the app side's
   *  facts, asked for once when it first opens. */
  let closeDiagnostics: (() => void) | null = null;
  let system: AppSystemInfo | null = null;
  const gatherDiagnostics = (): DiagnosticsInfo => {
    const graphics = viewport.graphicsInfo();
    const rb = current;
    return {
      generated: new Date().toISOString(),
      app: {
        version: system?.appVersion ?? null,
        uiBuild: typeof __UI_BUILD__ === "string" ? __UI_BUILD__ : "unknown",
        webkitSetup: system?.webkitSetup ?? null,
      },
      system,
      browser: {
        userAgent: navigator.userAgent,
        devicePixelRatio: window.devicePixelRatio,
        window: [window.innerWidth, window.innerHeight],
        cpuThreads: navigator.hardwareConcurrency,
      },
      webgl: graphics.webgl,
      program: rb
        ? {
            name: file.name,
            lines: editor.lineCount(),
            segments: rb.header.segmentCount,
            vertices: rb.header.vertexCount,
            bufferBytes: lastRun?.bytes ?? 0,
            diagnostics: rb.header.diagCount,
            machine: machineSelect.selectedOptions[0]?.textContent ?? "",
            dialect: dialectSelect.selectedOptions[0]?.textContent ?? "",
          }
        : null,
      lastRun,
      gpu: graphics.gpu,
      timings: [
        { label: "Key press waiting (input delay)", summary: perf.inputDelay.summary() },
        { label: "Keyboard repeat interval (keyboard's timestamps)", summary: perf.keyRepeat.summary() },
        { label: "Held key's repeats reaching the page (page's clock)", summary: perf.keyArrival.summary() },
        { label: "Page handling one key press (editor)", summary: perf.keyHandling.summary() },
        { label: "Paced steps for a held cursor key", summary: perf.pacedStep.summary() },
        { label: "Follow cursor (marker, readout, highlight, status)", summary: perf.follow.summary() },
        { label: "3D draw (JavaScript side)", summary: perf.render.summary() },
        { label: "Frame interval (while this panel is open)", summary: perf.frame.summary() },
      ],
      heldBack: perf.heldBack,
      lastHold: keyHold.last,
      switches: diagnosticSwitches.map((w) => ({ label: w.label, on: w.get() })),
    };
  };
  /** Settings to try from the panel (M5.2), not remembered. */
  const diagnosticSwitches: Switch[] = [
    {
      label: "Pace held cursor keys",
      get: () => editor.paced.enabled,
      set: (on) => {
        editor.paced.enabled = on;
      },
    },
    {
      label: "3D view at half resolution",
      get: () => viewport.halfResolution,
      set: (on) => {
        viewport.setHalfResolution(on);
      },
    },
  ];
  let lastPacedStep = Number.NEGATIVE_INFINITY;
  editor.onPacedStep = () => {
    const now = performance.now();
    if (now - lastPacedStep < 1000) perf.pacedStep.add(now - lastPacedStep);
    lastPacedStep = now;
  };
  const toggleDiagnostics = (): void => {
    if (closeDiagnostics) {
      closeDiagnostics();
      return;
    }
    closeDiagnostics = showDiagnostics(
      gatherDiagnostics,
      perf,
      () => {
        closeDiagnostics = null;
      },
      diagnosticSwitches,
    );
    if (!system && inApp()) {
      void systemInfo()
        .then((info) => {
          system = info;
        })
        .catch(() => undefined);
    }
  };
  editor.onCursorLine = followCursor;
  editor.onSelectedLines = followCursor;
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
  // In the capture phase: the editor, which gets the focus back while the
  // dialog steps aside, handles Escape itself.
  window.addEventListener(
    "keydown",
    (e) => {
      if (e.key === "Escape" && pickForTransform) {
        e.preventDefault();
        e.stopPropagation();
        pickForTransform(null);
      }
    },
    { capture: true },
  );
  viewport.onPick = (p) => {
    if (pickForTransform) {
      if (p) pickForTransform(p);
      return;
    }
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
    // The machine's control speaks its own dialect (M5.6b): choosing the
    // machine sets it; the dialect list can still override it.
    const dialect = machineDialect(usable[Number(machineSelect.value)], dialects);
    if (dialect !== null) dialectSelect.value = dialect;
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
    panel.setNotes([]);
    // Another file: every tool shown again.
    hiddenTools = new Set();
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

  /** Transform… (M4.9): the selected lines, rewritten by the core. */
  const openTransform = (): void => {
    const { first, last } = editor.selectedLines();
    showTransformDialog(first, last, {
      apply: async (form) => {
        const built = buildRequest(first, last, form);
        if (!built.ok) return built.error;
        const program = new TextEncoder().encode(editor.text());
        let list;
        try {
          list = decodeEdits(await transformProgram(program, built.request));
        } catch (e) {
          return String(e);
        }
        if (list.edits.length === 0) {
          return "Nothing changes: the selected lines are already where this would put them.";
        }
        editor.applyLineChanges(editorChanges(list.edits, (n) => editor.lineText(n)));
        panel.setNotes(list.notes);
        const where = first === last ? `line ${String(first)}` : `lines ${String(first)}\u2013${String(last)}`;
        notice = `Transformed ${where}: ${String(list.edits.length)} change${list.edits.length === 1 ? "" : "s"} (one Undo reverts it).`;
        void runner.runNow();
        return null;
      },
      pick: async () => {
        if (!current || freshness.stale) {
          throw new Error("refresh the view first (F5), then pick");
        }
        const rb = current;
        setStatus("Click a node in the view for the centre (Esc to cancel).");
        const picked = await new Promise<ToolPoint | null>((resolve) => {
          pickForTransform = (p) => {
            pickForTransform = null;
            resolve(p);
          };
        });
        setStatus("");
        if (!picked) return null;
        const line = rb.segment(picked.segment).srcLine;
        const values = await programPoint(new TextEncoder().encode(editor.text()), line, picked.point);
        const of = (letter: string): number => values.find((v) => v.letter === letter)?.value ?? 0;
        return { X: of("X"), Y: of("Y"), Z: of("Z") };
      },
    });
  };
  transformButton.addEventListener("click", openTransform);

  await applySettings();
  // A file the app was started with (opened from the file manager, M4.11)
  // replaces the sample; loading it runs the program.
  const startup = await startupFile().catch(() => null);
  if (startup !== null) {
    const result = await openPath(startup, loadFile);
    if (result.ok) {
      load(result.text, {
        path: startup,
        name: baseName(startup),
        ending: detectLineEnding(result.text),
        notUtf8: result.notUtf8,
      });
      if (result.notUtf8) notice = `${baseName(startup)} is not UTF-8 text: some characters are shown as \u{FFFD}.`;
      return;
    }
    notice = `Could not read ${result.path}: ${result.error}`;
  }
  await runner.runNow();
}

const app = document.getElementById("app");
if (app) {
  main(app).catch((e: unknown) => {
    setStatus(`Error: ${String(e)}`);
  });
}
