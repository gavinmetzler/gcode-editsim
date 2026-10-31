// The page's panes on screen: splitters to resize them by dragging, and
// maximising one pane to fill the window. The sizes and toggles come from
// the pure functions in panes.ts.

import { el } from "../dom";
import { splitFraction, toggleMaximised, type PaneName } from "./panes";

/**
 * Puts a splitter before `after` inside `container`. Dragging it gives the
 * first pane the share of `container` up to the pointer, through
 * `apply(share)` (0 to 1). `axis` "x" splits side by side, "y" one above
 * the other.
 */
export function addSplitter(
  container: HTMLElement,
  after: HTMLElement,
  axis: "x" | "y",
  apply: (share: number) => void,
): HTMLElement {
  const bar = el("div", { className: `split split-${axis}` });
  bar.setAttribute("role", "separator");
  bar.setAttribute("aria-orientation", axis === "x" ? "vertical" : "horizontal");
  bar.title = "Drag to resize";
  container.insertBefore(bar, after);
  const move = (e: PointerEvent): void => {
    const box = container.getBoundingClientRect();
    const share =
      axis === "x" ? splitFraction(e.clientX - box.left, box.width) : splitFraction(e.clientY - box.top, box.height);
    apply(share);
  };
  bar.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    bar.setPointerCapture(e.pointerId);
    bar.classList.add("dragging");
    bar.addEventListener("pointermove", move);
  });
  const end = (e: PointerEvent): void => {
    if (bar.hasPointerCapture(e.pointerId)) bar.releasePointerCapture(e.pointerId);
    bar.classList.remove("dragging");
    bar.removeEventListener("pointermove", move);
  };
  bar.addEventListener("pointerup", end);
  bar.addEventListener("pointercancel", end);
  return bar;
}

/**
 * Maximises one pane at a time. The layout shows it through
 * `data-maximised` on `root` (the stylesheet hides the rest), and each
 * maximised pane shows its exit button.
 */
export class Maximiser {
  private current: PaneName | null = null;
  private readonly exits = new Map<PaneName, HTMLButtonElement>();

  /** Called after every change, with the maximised pane or null. */
  onChange: (pane: PaneName | null) => void = () => undefined;

  constructor(private readonly root: HTMLElement) {}

  /** The maximised pane, or null for the normal layout. */
  get maximised(): PaneName | null {
    return this.current;
  }

  /** Adds an "Exit full screen" button to `box`, shown while `pane` is
   *  maximised, and returns it. */
  addExitButton(pane: PaneName, box: HTMLElement): HTMLButtonElement {
    const button = el("button", {
      type: "button",
      className: "exit-full",
      textContent: "Exit full screen",
      title: "Back to the normal layout (or double-click)",
    });
    button.hidden = true;
    button.addEventListener("click", () => {
      this.set(null);
    });
    this.exits.set(pane, button);
    box.append(button);
    return button;
  }

  /** Maximises `pane`, or restores if it is already maximised. */
  toggle(pane: PaneName): void {
    this.set(toggleMaximised(this.current, pane));
  }

  /** Maximises `pane`, or restores the normal layout for null. */
  set(pane: PaneName | null): void {
    this.current = pane;
    if (pane === null) delete this.root.dataset.maximised;
    else this.root.dataset.maximised = pane;
    for (const [name, button] of this.exits) button.hidden = name !== pane;
    this.onChange(pane);
  }
}

/** Whether a double-click on `target` should toggle maximising: not on
 *  the pane's buttons, inputs or labels, which have their own clicks. */
export function dblclickToggles(target: EventTarget | null): boolean {
  return !(target instanceof Element && target.closest("button, input, select, label, a") !== null);
}
