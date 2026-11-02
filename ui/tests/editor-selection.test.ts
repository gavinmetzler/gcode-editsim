import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

// M5.2: holding Shift+Page Up/Down was slow in the desktop app. Measured in
// the real app (WebKitGTK), CodeMirror's `drawSelection`, which draws the
// selection and cursor itself and measures the selection's rectangles on
// every move, kept the editor busy for about a second after the key was
// released on a large selection; without it, about a third of a second.
// The editor uses the browser's own selection and caret instead, styled in
// the same colours. (Tests run without a browser, so this reads the
// source; the timings are in the PR.)

const source = readFileSync(new URL("../src/editor/editor.ts", import.meta.url), "utf8");

describe("the editor's selection", () => {
  it("is drawn by the browser, not by CodeMirror's drawSelection", () => {
    expect(source).not.toMatch(/drawSelection\s*\(/);
  });

  it("keeps the dark theme's selection colour and a light caret", () => {
    expect(source).toMatch(/"[^"]*::selection[^"]*":\s*\{\s*backgroundColor:\s*"#3a4a63"/);
    expect(source).toMatch(/caretColor:\s*"#e6e6e6"/);
  });
});
