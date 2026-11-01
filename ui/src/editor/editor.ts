// The program editor (CodeMirror 6): basic text editing with history and
// search, G-code highlighting, diagnostics in the gutter, and callbacks for
// edits and for the line the cursor is on.

import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { lintGutter, setDiagnostics, type Diagnostic as LintDiagnostic } from "@codemirror/lint";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { EditorSelection, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";

import { gcodeLanguage } from "./gcode-language";
import { selectedLines as wholeLines, type LineMark, type LineRange } from "./lines";

const highlight = HighlightStyle.define([
  { tag: tags.comment, color: "#7f8c8d", fontStyle: "italic" },
  { tag: tags.keyword, color: "#e5a95b", fontWeight: "bold" },
  { tag: tags.propertyName, color: "#7cb7ff" },
  { tag: tags.number, color: "#b5d68d" },
  { tag: tags.meta, color: "#c792ea" },
  { tag: tags.variableName, color: "#f07178" },
]);

const theme = EditorView.theme(
  {
    "&": { height: "100%", backgroundColor: "#14171c", color: "#e6e6e6" },
    ".cm-scroller": { fontFamily: "ui-monospace, monospace", fontSize: "13px" },
    ".cm-gutters": { backgroundColor: "#1d2128", color: "#7a828c", border: "none" },
    ".cm-activeLine": { backgroundColor: "#ffffff0d" },
    ".cm-activeLineGutter": { backgroundColor: "#ffffff14" },
    ".cm-cursor": { borderLeftColor: "#e6e6e6" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "#3a4a63" },
  },
  { dark: true },
);

/** A G-code editor inside `parent`. */
export class GcodeEditor {
  private readonly view: EditorView;
  private readonly extensions: Extension[];
  private cursorLine = 0;

  /** Called after every edit of the text. */
  onChange: () => void = () => undefined;
  /** Called when the cursor moves to another line (1-based). */
  onCursorLine: (line: number) => void = () => undefined;
  /** Called when the selected lines change: whole lines, the cursor's
   *  own line when nothing is selected (M4.3). */
  onSelectedLines: (range: LineRange) => void = () => undefined;
  private selected: LineRange = { first: 0, last: 0 };

  constructor(parent: HTMLElement, text: string) {
    this.extensions = [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          highlightSelectionMatches(),
          lintGutter(),
          gcodeLanguage,
          syntaxHighlighting(highlight),
          theme,
          keymap.of([...defaultKeymap, ...historyKeymap, ...searchKeymap, indentWithTab]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) this.onChange();
            if (update.docChanged || update.selectionSet) {
              const line = update.state.doc.lineAt(update.state.selection.main.head).number;
              if (line !== this.cursorLine) {
                this.cursorLine = line;
                this.onCursorLine(line);
              }
              const range = this.selectedLines();
              if (range.first !== this.selected.first || range.last !== this.selected.last) {
                this.selected = range;
                this.onSelectedLines(range);
              }
            }
          }),
    ];
    this.view = new EditorView({ parent, state: EditorState.create({ doc: text, extensions: this.extensions }) });
  }

  /** The text, with lines joined by "\n" whatever the original line endings
   *  were, so its line numbers are the editor's (the parser splits on "\n"). */
  text(): string {
    return this.view.state.doc.toString();
  }

  /** Replaces the whole text (a newly opened file), clearing undo history. */
  setText(text: string): void {
    this.cursorLine = 0;
    this.selected = { first: 0, last: 0 };
    this.view.setState(EditorState.create({ doc: text, extensions: this.extensions }));
  }

  /** The whole lines the main selection covers (the cursor's line when
   *  nothing is selected). */
  selectedLines(): LineRange {
    const { doc, selection } = this.view.state;
    const { from, to } = selection.main;
    const end = doc.lineAt(to);
    return wholeLines(doc.lineAt(from).number, end.number, to === end.from);
  }

  /** The 1-based line the cursor is on. */
  cursorLineNumber(): number {
    const state = this.view.state;
    return state.doc.lineAt(state.selection.main.head).number;
  }

  /** Number of lines in the text. */
  lineCount(): number {
    return this.view.state.doc.lines;
  }

  /** Puts the cursor at the start of `line` (1-based) and scrolls it into view. */
  revealLine(line: number): void {
    const doc = this.view.state.doc;
    if (line < 1 || line > doc.lines) return;
    const at = doc.line(line).from;
    this.view.dispatch({
      selection: EditorSelection.cursor(at),
      effects: EditorView.scrollIntoView(at, { y: "center" }),
    });
  }

  /** Shows diagnostics in the gutter and underlines their lines. */
  setMarks(marks: readonly LineMark[]): void {
    const doc = this.view.state.doc;
    const diagnostics: LintDiagnostic[] = marks
      .filter((m) => m.line >= 1 && m.line <= doc.lines)
      .map((m) => {
        const line = doc.line(m.line);
        return { from: line.from, to: line.to, severity: m.severity, message: m.message };
      });
    this.view.dispatch(setDiagnostics(this.view.state, diagnostics));
  }
}
