// The transform edit list from the app (app/backend/src/transform.rs,
// layout version 1): decoded with every length checked, and turned into
// editor changes. No G-code meaning here: the core chose the edits.

/** A damaged edit list. */
export class EditListError extends Error {}

/** Replace bytes `start..end` of `line` (1-based) with `text`. */
export interface Edit {
  readonly line: number;
  readonly start: number;
  readonly end: number;
  readonly text: string;
}

/** Something the user should know about the transformation. */
export interface TransformNote {
  readonly severity: "info" | "warning" | "error";
  readonly line: number;
  readonly message: string;
}

/** A decoded edit list. */
export interface EditList {
  readonly edits: Edit[];
  readonly notes: TransformNote[];
}

/** "GCTE" read as a little-endian u32. */
const MAGIC = 0x45544347;
const VERSION = 1;
const SEVERITIES = ["info", "warning", "error"] as const;

/** Decodes an edit list; throws `EditListError` if it is damaged. */
export function decodeEdits(buffer: ArrayBuffer): EditList {
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);
  const utf8 = new TextDecoder("utf-8", { fatal: true });
  let at = 0;
  const u32 = (): number => {
    if (at + 4 > buffer.byteLength) throw new EditListError("edit list cut short");
    const v = view.getUint32(at, true);
    at += 4;
    return v;
  };
  const text = (length: number): string => {
    if (at + length > buffer.byteLength) throw new EditListError("edit list cut short");
    let s: string;
    try {
      s = utf8.decode(bytes.subarray(at, at + length));
    } catch {
      throw new EditListError("edit list text is not UTF-8");
    }
    at = Math.ceil((at + length) / 4) * 4;
    if (at > buffer.byteLength) throw new EditListError("edit list cut short");
    return s;
  };
  if (u32() !== MAGIC) throw new EditListError("not an edit list");
  if ((u32() & 0xffff) !== VERSION) throw new EditListError("edit list of another version");
  const editCount = u32();
  const noteCount = u32();
  const edits: Edit[] = [];
  for (let i = 0; i < editCount; i += 1) {
    const line = u32();
    const start = u32();
    const end = u32();
    edits.push({ line, start, end, text: text(u32()) });
  }
  const notes: TransformNote[] = [];
  for (let i = 0; i < noteCount; i += 1) {
    if (at + 4 > buffer.byteLength) throw new EditListError("edit list cut short");
    const severity = SEVERITIES[bytes[at] ?? 255];
    if (severity === undefined) throw new EditListError("unknown note severity");
    at += 4;
    const line = u32();
    notes.push({ severity, line, message: text(u32()) });
  }
  if (at !== buffer.byteLength) throw new EditListError("edit list has bytes left over");
  return { edits, notes };
}

/** The UTF-16 offset in `line` of UTF-8 byte offset `byte` (clamped to
 *  the line). */
export function charOffset(line: string, byte: number): number {
  let bytes = 0;
  let index = 0;
  for (const ch of line) {
    if (bytes >= byte) break;
    const code = ch.codePointAt(0) ?? 0;
    bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
    index += ch.length;
  }
  return index;
}

/** A change to one editor line, in UTF-16 offsets within the line. */
export interface LineChange {
  readonly line: number;
  readonly from: number;
  readonly to: number;
  readonly insert: string;
}

/** The edits as editor changes, `lineText(n)` giving line n's text (null
 *  for a line the editor does not have, which is an error: the text
 *  changed since the edits were made). */
export function editorChanges(edits: readonly Edit[], lineText: (line: number) => string | null): LineChange[] {
  return edits.map((e) => {
    const text = lineText(e.line);
    if (text === null) throw new Error(`line ${String(e.line)} is not in the editor`);
    return { line: e.line, from: charOffset(text, e.start), to: charOffset(text, e.end), insert: e.text };
  });
}
