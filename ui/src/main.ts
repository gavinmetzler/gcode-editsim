// The UI: a toolbar, the toolpath viewport and a status line. M2.5 shows a
// program's toolpath; the editor (M2.6) and panels (M2.7) come next.
//
// No G-code meaning here: the core interprets, the UI draws what it got.

import "./style.css";

import sample from "../../tests/golden/basic_mill.nc?raw";
import { decodeRenderBuffer, type RenderBuffer } from "./buffer/decode";
import {
  inApp,
  interpret,
  listDialects,
  listMachines,
  selectMachine,
  setRunOptions,
  type MachineEntry,
} from "./ipc/backend";
import { buildToolpath } from "./viewport/toolpath";
import { Viewport } from "./viewport/viewport";
import { VIEWS, type ViewName } from "./viewport/views";

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

const status = el("span", { className: "status" });
const setStatus = (text: string): void => {
  status.textContent = text;
};

async function main(app: HTMLElement): Promise<void> {
  const machineSelect = el("select", { title: "Machine" });
  const dialectSelect = el("select", { title: "Dialect" });
  const fileInput = el("input", { type: "file", accept: ".nc,.ngc,.tap,.txt,.cnc,.mpf,.iso,*" });
  const openButton = el("button", { type: "button", textContent: "Open…" });
  const sampleButton = el("button", { type: "button", textContent: "Sample" });
  const viewButtons = (Object.keys(VIEWS) as ViewName[]).map((name) =>
    el("button", { type: "button", textContent: VIEWS[name].label, className: "view" }),
  );
  const fitButton = el("button", { type: "button", textContent: "Fit" });
  const viewportBox = el("div", { className: "viewport" });
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
      el("span", { className: "spacer" }),
      ...viewButtons,
      fitButton,
    ),
    viewportBox,
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

  const viewport = new Viewport(viewportBox);
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
    return;
  }

  let current: RenderBuffer | null = null;
  let program: { bytes: Uint8Array; name: string } = {
    bytes: new TextEncoder().encode(sample),
    name: "basic_mill.nc (sample)",
  };

  const run = async (): Promise<void> => {
    try {
      setStatus(`Interpreting ${program.name}…`);
      const rb = decodeRenderBuffer(await interpret(program.bytes, program.name));
      current = rb;
      viewport.show(buildToolpath(rb));
      const d = rb.header.diagCount;
      setStatus(
        `${program.name}: ${String(rb.header.segmentCount)} segments, ${String(d)} diagnostic${d === 1 ? "" : "s"}. Click the toolpath to see a segment's line.`,
      );
    } catch (e) {
      current = null;
      setStatus(`Could not show ${program.name}: ${String(e)}`);
    }
  };

  viewport.onPick = (pick) => {
    if (!current || !pick) return;
    const s = current.segment(pick.segment);
    setStatus(`Line ${String(s.srcLine)} (segment ${String(pick.segment + 1)} of ${String(current.header.segmentCount)})`);
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
  machineSelect.addEventListener("change", () => {
    void applySettings().then(run);
  });
  dialectSelect.addEventListener("change", () => {
    void applySettings().then(run);
  });
  openButton.addEventListener("click", () => {
    fileInput.click();
  });
  fileInput.addEventListener("change", () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    void file.arrayBuffer().then((buffer) => {
      program = { bytes: new Uint8Array(buffer), name: file.name };
      fileInput.value = "";
      return run();
    });
  });
  sampleButton.addEventListener("click", () => {
    program = { bytes: new TextEncoder().encode(sample), name: "basic_mill.nc (sample)" };
    void run();
  });

  await applySettings();
  await run();
}

const app = document.getElementById("app");
if (app) {
  main(app).catch((e: unknown) => {
    setStatus(`Error: ${String(e)}`);
  });
}
