// The transform edit list (M4.9): decoded from the same fixture the Rust
// side writes (app/backend/tests/transform.rs), and turned into editor
// changes. Spans are bytes in the line; the editor counts UTF-16 units.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { charOffset, decodeEdits, EditListError, editorChanges } from "../src/transform/edits";

function fixture(): ArrayBuffer {
  const path = fileURLToPath(new URL("../../tests/golden/transform_edits_v1.bin", import.meta.url));
  const bytes = readFileSync(path);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

describe("decodeEdits", () => {
  it("reads the shared fixture", () => {
    expect(decodeEdits(fixture())).toEqual({
      edits: [
        { line: 2, start: 4, end: 7, text: "11." },
        { line: 2, start: 11, end: 11, text: " Y2." },
        { line: 5, start: 9, end: 12, text: "-é5" },
      ],
      notes: [
        { severity: "info", line: 2, message: "first move" },
        { severity: "warning", line: 6, message: "after the selection" },
      ],
    });
  });

  it("refuses a damaged list", () => {
    const good = new Uint8Array(fixture());
    for (const cut of [0, 3, 15, 20, good.length - 1]) {
      expect(() => decodeEdits(good.slice(0, cut).buffer), `cut ${String(cut)}`).toThrow(EditListError);
    }
    const bad = good.slice();
    bad[0] = 0;
    expect(() => decodeEdits(bad.buffer)).toThrow(EditListError);
    const extra = new Uint8Array(good.length + 4);
    extra.set(good);
    expect(() => decodeEdits(extra.buffer)).toThrow(EditListError);
  });
});

describe("charOffset", () => {
  it("is the byte offset for plain ASCII", () => {
    expect(charOffset("G1 X10.", 4)).toBe(4);
  });

  it("counts characters, not bytes, after accented letters", () => {
    // "ü" is 2 bytes in UTF-8 and 1 UTF-16 unit.
    expect(charOffset("(ü) X10.", 6)).toBe(5);
    // An emoji is 4 bytes and 2 UTF-16 units.
    expect(charOffset("(\u{1F600}) X1.", 7)).toBe(5);
  });

  it("clamps to the line", () => {
    expect(charOffset("X1.", 99)).toBe(3);
    expect(charOffset("X1.", -1)).toBe(0);
  });
});

describe("editorChanges", () => {
  it("maps each edit to its line and character offsets", () => {
    const lines = ["G21 G90", "G1 X10. Y0.", "(ü) X1."];
    const changes = editorChanges(
      [
        { line: 2, start: 4, end: 7, text: "11." },
        { line: 3, start: 6, end: 8, text: "2." },
      ],
      (n) => lines[n - 1] ?? null,
    );
    expect(changes).toEqual([
      { line: 2, from: 4, to: 7, insert: "11." },
      { line: 3, from: 5, to: 7, insert: "2." },
    ]);
  });

  it("refuses an edit for a line the editor does not have", () => {
    expect(() => editorChanges([{ line: 9, start: 0, end: 0, text: "x" }], () => null)).toThrow();
  });
});
