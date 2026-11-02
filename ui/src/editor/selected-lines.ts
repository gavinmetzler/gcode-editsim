// The lines a multi-line selection covers, marked with a full-width
// background that stays when the editor loses focus (M5.2). The browser
// draws the selection itself (CodeMirror's drawSelection was too slow on
// large selections in WebKitGTK), and its selection is hidden while the
// editor is not focused. Whole lines are also what the 3D highlight and
// Transform act on. Only the lines on screen are decorated, and nothing is
// measured, so a selection of the whole program costs no more than a page.

import { type Extension, RangeSetBuilder } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

import { selectedLines } from "./lines";

/** First and last line (1-based, inclusive). */
export interface LineSpan {
  readonly first: number;
  readonly last: number;
}

/** The lines to mark: those of `selection` (when it spans more than one
 *  line) within the `visible` spans, ascending and without repeats. */
export function linesToMark(selection: LineSpan | null, visible: readonly LineSpan[]): number[] {
  if (!selection || selection.last <= selection.first) return [];
  const lines: number[] = [];
  let next = selection.first;
  for (const span of visible) {
    const from = Math.max(next, span.first);
    const to = Math.min(selection.last, span.last);
    for (let line = from; line <= to; line += 1) lines.push(line);
    next = Math.max(next, to + 1);
  }
  return lines;
}

const mark = Decoration.line({ class: "cm-selectedLine" });

function build(view: EditorView): DecorationSet {
  const { doc, selection } = view.state;
  const main = selection.main;
  // The same whole lines the 3D highlight and Transform use.
  const end = doc.lineAt(main.to);
  const span = main.empty ? null : selectedLines(doc.lineAt(main.from).number, end.number, main.to === end.from);
  const visible = view.visibleRanges.map((r) => ({ first: doc.lineAt(r.from).number, last: doc.lineAt(r.to).number }));
  const builder = new RangeSetBuilder<Decoration>();
  for (const line of linesToMark(span, visible)) {
    const at = doc.line(line).from;
    builder.add(at, at, mark);
  }
  return builder.finish();
}

/** Marks the selected lines (see the module comment). */
export const markSelectedLines: Extension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = build(view);
    }
    update(update: ViewUpdate): void {
      if (update.docChanged || update.selectionSet || update.viewportChanged) this.decorations = build(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
