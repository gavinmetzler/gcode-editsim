import { StringStream } from "@codemirror/language";
import { describe, expect, it } from "vitest";

import { gcodeToken } from "../src/editor/gcode-language";

/** The tokens of one line as [text, style] pairs (spaces left out). */
function tokens(line: string): [string, string | null][] {
  const stream = new StringStream(line, 4, 2);
  const out: [string, string | null][] = [];
  while (!stream.eol()) {
    const style = gcodeToken(stream);
    const text = stream.current();
    if (text.trim() !== "") out.push([text, style]);
    stream.start = stream.pos;
  }
  return out;
}

describe("gcodeToken", () => {
  it("styles words by type only", () => {
    expect(tokens("N10 G01 X-1.5 F200.")).toEqual([
      ["N10", "meta"],
      ["G", "keyword"],
      ["01", "number"],
      ["X", "propertyName"],
      ["-1.5", "number"],
      ["F", "propertyName"],
      ["200.", "number"],
    ]);
  });

  it("styles both comment forms to the right place", () => {
    expect(tokens("G0 (move) X1 ; to start")).toEqual([
      ["G", "keyword"],
      ["0", "number"],
      ["(move)", "comment"],
      ["X", "propertyName"],
      ["1", "number"],
      ["; to start", "comment"],
    ]);
    // An unclosed parenthesis runs to the end of the line.
    expect(tokens("(open")).toEqual([["(open", "comment"]]);
  });

  it("styles block delete, program markers and variables", () => {
    expect(tokens("/M01")).toEqual([
      ["/", "meta"],
      ["M", "keyword"],
      ["01", "number"],
    ]);
    expect(tokens("%")).toEqual([["%", "meta"]]);
    expect(tokens("X#100")).toEqual([
      ["X", "propertyName"],
      ["#100", "variableName"],
    ]);
  });

  it("never stalls on any character", () => {
    const line = Array.from({ length: 128 }, (_, i) => String.fromCharCode(i)).join("");
    expect(tokens(line).length).toBeGreaterThan(0);
  });
});
