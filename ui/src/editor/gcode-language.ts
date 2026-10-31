// Lightweight G-code highlighting for the editor: colours by token type
// only (comments, G and M words, other address letters, numbers, sequence
// numbers, variables). It gives the code no meaning; the core does that.

import { StreamLanguage, type StreamParser, type StringStream } from "@codemirror/language";

/** The tokenizer keeps no state between lines. */
type State = Record<string, never>;

const NUMBER = /^[-+]?(?:\d+\.?\d*|\.\d+)/;

/** Reads one token and returns its style name, or null for plain text. */
export function gcodeToken(stream: StringStream): string | null {
  if (stream.eatSpace()) return null;
  const ch = stream.peek() ?? "";
  if (ch === ";") {
    stream.skipToEnd();
    return "comment";
  }
  if (ch === "(") {
    stream.next();
    while (!stream.eol()) {
      if (stream.next() === ")") break;
    }
    return "comment";
  }
  if (ch === "%" || (ch === "/" && stream.column() === stream.indentation())) {
    stream.next();
    stream.match(/^\d/);
    return "meta";
  }
  if (ch === "#") {
    stream.next();
    stream.match(/^\d+/);
    return "variableName";
  }
  if (/[A-Za-z]/.test(ch)) {
    const letter = (stream.next() ?? "").toUpperCase();
    if (letter === "N" && stream.match(/^\d+/)) return "meta";
    if (letter === "G" || letter === "M") return "keyword";
    return "propertyName";
  }
  if (stream.match(NUMBER)) return "number";
  stream.next();
  return null;
}

const parser: StreamParser<State> = {
  name: "gcode",
  startState: () => ({}),
  token: gcodeToken,
  languageData: { commentTokens: { line: ";" } },
};

/** The G-code language for CodeMirror. */
export const gcodeLanguage = StreamLanguage.define(parser);
