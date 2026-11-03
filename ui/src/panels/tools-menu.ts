// The Tools ▾ menu in the toolbar (M5.4): a checkbox per tool in the
// program, in the order the tools are first used, with All and None;
// right-clicking a tool shows only that one. It only reports which tools
// are hidden; the view does the filtering (viewport/tool-filter.ts).

import { el } from "../dom";
import { onlyTool, toolsButtonLabel } from "../viewport/tool-filter";
import { toolName } from "./stats";

/** See the module comment. */
export class ToolsMenu {
  /** The toolbar button that opens the menu. */
  readonly button = el("button", {
    type: "button",
    textContent: toolsButtonLabel(0, 0),
    title: "Show or hide each tool's moves. Right-click a tool to show only that one.",
    disabled: true,
  });
  private readonly list = el("div", { className: "tools-menu" });
  private tools: number[] = [];
  private hidden = new Set<number>();

  /** `onChange` gets the hidden tools after every change. */
  constructor(private readonly onChange: (hidden: ReadonlySet<number>) => void) {
    this.list.hidden = true;
    document.body.append(this.list);
    this.button.addEventListener("click", () => {
      if (this.list.hidden) this.open();
      else this.close();
    });
    // Close on a click outside, or Escape.
    document.addEventListener("mousedown", (e) => {
      const target = e.target as Node | null;
      if (!this.list.hidden && target && !this.list.contains(target) && !this.button.contains(target)) this.close();
    });
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape" && !this.list.hidden) this.close();
    });
  }

  /** The program's tools (in order of first use) and the hidden ones. */
  setTools(tools: readonly number[], hidden: ReadonlySet<number>): void {
    this.tools = [...tools];
    this.hidden = new Set(hidden);
    this.button.disabled = this.tools.length === 0;
    this.render();
  }

  private set(hidden: Set<number>): void {
    this.hidden = hidden;
    this.render();
    this.onChange(this.hidden);
  }

  private open(): void {
    const r = this.button.getBoundingClientRect();
    this.list.style.left = `${String(Math.round(r.left))}px`;
    this.list.style.top = `${String(Math.round(r.bottom + 2))}px`;
    this.list.hidden = false;
  }

  private close(): void {
    this.list.hidden = true;
  }

  private render(): void {
    this.button.textContent = toolsButtonLabel(this.tools.length, this.hidden.size);
    const all = el("button", { type: "button", textContent: "All" });
    const none = el("button", { type: "button", textContent: "None" });
    all.addEventListener("click", () => {
      this.set(new Set());
    });
    none.addEventListener("click", () => {
      this.set(new Set(this.tools));
    });
    const rows = this.tools.map((tool) => {
      const box = el("input", { type: "checkbox", checked: !this.hidden.has(tool) });
      box.addEventListener("change", () => {
        const next = new Set(this.hidden);
        if (box.checked) next.delete(tool);
        else next.add(tool);
        this.set(next);
      });
      const row = el("label", { className: "tool", title: "Right-click: show only this tool" }, box, ` ${toolName(tool)}`);
      row.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        this.set(onlyTool(this.tools, tool));
      });
      return row;
    });
    this.list.replaceChildren(el("div", { className: "buttons" }, all, none), ...rows);
  }
}
