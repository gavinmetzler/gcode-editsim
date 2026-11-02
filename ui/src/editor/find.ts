// Find and Replace (M5.3): CodeMirror's own search panel, plain text
// (not G-code aware), made easy to find and drawn in the app's dark style.
//
// - Ctrl+F opens it (CodeMirror), Ctrl+H opens it at the Replace field;
//   Enter and F3 go to the next match, Shift+F3 to the previous one, also
//   with the panel closed (the last search is kept). Escape closes it.
// - A match is scrolled to about the middle of the editor, not its bottom
//   edge (owner, M5_PLAN §6 item 4).
// - The labels are capitalised like the app's buttons.

import type { KeyBinding } from "@codemirror/view";
import { EditorView } from "@codemirror/view";
import type { SelectionRange } from "@codemirror/state";

/** The Find… button's tooltip. */
export const FIND_TOOLTIP =
  "Find and replace text in the program (Ctrl+F; Ctrl+H to replace). " +
  "Enter or F3: next match, Shift+F3: previous. Esc closes it.";

/** The search panel's labels, as the app writes them. */
export const SEARCH_PHRASES: Readonly<Record<string, string>> = {
  next: "Next",
  previous: "Previous",
  all: "All",
  "match case": "Match case",
  regexp: "Regular expression",
  "by word": "By word",
  replace: "Replace",
  "replace all": "Replace all",
  close: "Close",
};

/** Ctrl+H: open the panel at the Replace field. `open(replace)` opens it. */
export function findKeymap(open: (replace: boolean) => boolean): KeyBinding[] {
  return [{ key: "Mod-h", run: () => open(true), preventDefault: true }];
}

/** Scrolls a match to about the middle of the editor. */
export function centerMatch(range: SelectionRange): ReturnType<typeof EditorView.scrollIntoView> {
  return EditorView.scrollIntoView(range, { y: "center" });
}

/** The panel's dark style (plain data, so it can be tested). */
export const findThemeSpec: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  ".cm-panels": { backgroundColor: "#1d2128", color: "#e6e6e6" },
  ".cm-panels.cm-panels-bottom": { borderTop: "1px solid #3a404a" },
  ".cm-search": { padding: "6px 8px", fontSize: "12px", display: "flex", flexWrap: "wrap", alignItems: "center", gap: "4px 6px" },
  ".cm-search br": { flexBasis: "100%", height: "0" },
  ".cm-search label": { color: "#c8ccd2", display: "inline-flex", alignItems: "center", gap: "3px" },
  ".cm-search input[type=checkbox]": { accentColor: "#7cb7ff", margin: "0" },
  ".cm-textfield": {
    backgroundColor: "#14171c",
    color: "#e6e6e6",
    border: "1px solid #3a404a",
    borderRadius: "4px",
    padding: "3px 6px",
    fontFamily: "ui-monospace, monospace",
    fontSize: "12px",
    minWidth: "16em",
  },
  ".cm-textfield:focus": { outline: "none", borderColor: "#7cb7ff" },
  ".cm-button": {
    backgroundColor: "#2b3038",
    backgroundImage: "none",
    color: "#e6e6e6",
    border: "1px solid #4a5260",
    borderRadius: "4px",
    padding: "2px 10px",
    fontSize: "12px",
    margin: "0",
    cursor: "pointer",
  },
  ".cm-button:hover": { backgroundColor: "#353b45" },
  ".cm-button:active": { backgroundColor: "#3f4652", backgroundImage: "none" },
  ".cm-search button[name=close]": { color: "#9aa0a6", fontSize: "16px", top: "4px", right: "6px" },
  // Matches: amber, apart from the selection's blue and the 3D view's yellow.
  ".cm-searchMatch": { backgroundColor: "#d9a44a40", outline: "1px solid #d9a44a99" },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "#d9a44aa0" },
};
