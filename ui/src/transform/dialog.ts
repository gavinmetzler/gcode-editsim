// The Transform dialog (M4.9): Translate, Rotate or Scale the selected
// lines. It only collects values: the request is built by request.ts and
// the core does the G-code work. The centre can be typed or picked in the
// 3D view (the dialog steps aside while the user clicks a node).

import { el } from "../dom";
import { emptyForm, type PlaneName, type TransformForm, type TransformKind } from "./request";

/** What the dialog needs from the page. */
export interface TransformDialogHooks {
  /** Applies the form; resolves to null when done (the dialog closes) or
   *  to a message to show (the dialog stays open). */
  readonly apply: (form: TransformForm) => Promise<string | null>;
  /** Lets the user pick a point in the view; resolves to its program
   *  coordinates, or null if cancelled. */
  readonly pick: () => Promise<{ readonly X: number; readonly Y: number; readonly Z: number } | null>;
}

/** The last values, so the dialog opens as it was left. */
let remembered: TransformForm = emptyForm();

/** A number for a field: trailing zeros removed, at most 4 decimals. */
function fieldText(n: number): string {
  return String(Math.round(n * 10000) / 10000);
}

/** Opens the dialog for lines `first..last`. */
export function showTransformDialog(first: number, last: number, hooks: TransformDialogHooks): void {
  const lines = first === last ? `line ${String(first)}` : `lines ${String(first)}–${String(last)}`;
  const input = (value: string, label: string): HTMLInputElement =>
    el("input", { type: "text", value, inputMode: "decimal", size: 8, title: label, ariaLabel: label });
  const f = remembered;
  const dx = input(f.dx, "dX (mm)");
  const dy = input(f.dy, "dY (mm)");
  const dz = input(f.dz, "dZ (mm)");
  const angle = input(f.angle, "Angle (degrees)");
  const factor = input(f.factor, "Factor");
  const cx = input(f.cx, "Centre X (mm)");
  const cy = input(f.cy, "Centre Y (mm)");
  const cz = input(f.cz, "Centre Z (mm)");
  const plane = el(
    "select",
    { title: "Plane" },
    el("option", { value: "XY", textContent: "XY (G17)" }),
    el("option", { value: "ZX", textContent: "ZX (G18)" }),
    el("option", { value: "YZ", textContent: "YZ (G19)" }),
  );
  plane.value = f.plane;
  const kinds: [TransformKind, string][] = [
    ["translate", "Translate"],
    ["rotate", "Rotate"],
    ["scale", "Scale"],
  ];
  const radios = kinds.map(([kind, label]) => {
    const radio = el("input", { type: "radio", name: "transform-kind", value: kind, checked: kind === f.kind });
    return { kind, radio, label: el("label", {}, radio, label) };
  });
  const labelled = (label: string, ...fields: HTMLElement[]): HTMLElement =>
    el("div", { className: "row" }, el("span", { className: "label", textContent: label }), ...fields);
  const pickButton = el("button", { type: "button", textContent: "Pick in view", title: "Click a node in the 3D view" });
  const translateBox = el("div", {}, labelled("Offset (mm)", el("span", { textContent: "X" }), dx, el("span", { textContent: "Y" }), dy, el("span", { textContent: "Z" }), dz));
  const rotateBox = el(
    "div",
    {},
    labelled("Plane", plane),
    labelled("Angle (°)", angle, el("span", { className: "hint", textContent: "counter-clockwise" })),
  );
  const scaleBox = el(
    "div",
    {},
    labelled("Factor", factor, el("span", { className: "hint", textContent: "X and Y; 0.8 is 80 %; Z is never scaled" })),
  );
  const centreBox = labelled(
    "Centre (mm)",
    el("span", { textContent: "X" }),
    cx,
    el("span", { textContent: "Y" }),
    cy,
    el("span", { textContent: "Z" }),
    cz,
    pickButton,
  );
  const message = el("p", { className: "note" });
  const applyButton = el("button", { type: "button", textContent: "Apply" });
  const cancel = el("button", { type: "button", textContent: "Cancel" });
  const dialog = el(
    "dialog",
    { className: "app-dialog transform-dialog" },
    el("h2", { textContent: `Transform ${lines}` }),
    el("div", { className: "kinds" }, ...radios.map((r) => r.label)),
    translateBox,
    rotateBox,
    scaleBox,
    centreBox,
    el("p", {
      className: "hint",
      textContent:
        "Values are in mm, in the program's coordinates. Only the words that must change are rewritten; one Undo (Ctrl+Z) reverts it. Check the result in the view before running it on a machine.",
    }),
    message,
    el("div", { className: "buttons" }, applyButton, cancel),
  );

  const kind = (): TransformKind => radios.find((r) => r.radio.checked)?.kind ?? "translate";
  const form = (): TransformForm => ({
    kind: kind(),
    dx: dx.value,
    dy: dy.value,
    dz: dz.value,
    plane: plane.value as PlaneName,
    angle: angle.value,
    factor: factor.value,
    cx: cx.value,
    cy: cy.value,
    cz: cz.value,
  });
  const showKind = (): void => {
    const k = kind();
    translateBox.hidden = k !== "translate";
    rotateBox.hidden = k !== "rotate";
    scaleBox.hidden = k !== "scale";
    centreBox.hidden = k === "translate";
    message.textContent = "";
  };
  for (const r of radios) r.radio.addEventListener("change", showKind);
  showKind();

  // Removed only when finished (applied, cancelled, or Esc on the
  // dialog), never when it steps aside to let the user pick: the close
  // event comes later than the pick may finish.
  let finished = false;
  const finish = (): void => {
    finished = true;
    dialog.close();
  };
  pickButton.addEventListener("click", () => {
    dialog.close();
    void hooks
      .pick()
      .then((p) => {
        if (p) {
          cx.value = fieldText(p.X);
          cy.value = fieldText(p.Y);
          cz.value = fieldText(p.Z);
          message.textContent = "Centre picked from the view.";
        }
      })
      .catch((e: unknown) => {
        message.textContent = `Could not pick: ${String(e)}`;
      })
      .finally(() => {
        if (!finished && !dialog.open) dialog.showModal();
      });
  });
  applyButton.addEventListener("click", () => {
    remembered = form();
    applyButton.disabled = true;
    message.textContent = "Working…";
    void hooks
      .apply(remembered)
      .then((problem) => {
        if (problem === null) finish();
        else message.textContent = problem;
      })
      .catch((e: unknown) => {
        message.textContent = String(e);
      })
      .finally(() => {
        applyButton.disabled = false;
      });
  });
  cancel.addEventListener("click", () => {
    remembered = form();
    finish();
  });
  dialog.addEventListener("cancel", () => {
    // Esc on the dialog itself.
    remembered = form();
    finished = true;
  });
  dialog.addEventListener("close", () => {
    if (finished) dialog.remove();
  });
  document.body.append(dialog);
  dialog.showModal();
}
