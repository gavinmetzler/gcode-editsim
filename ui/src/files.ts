// Program text in and out of files: decoding, line endings and names.
// Pure, so it is tested without the app.

/** Decoded file contents. */
export interface Decoded {
  readonly text: string;
  /** Some bytes were not UTF-8 and became U+FFFD; saving would change them. */
  readonly notUtf8: boolean;
}

/** Decodes file bytes as UTF-8; bytes that are not UTF-8 become U+FFFD. */
export function decodeProgram(bytes: ArrayBuffer | Uint8Array): Decoded {
  const text = new TextDecoder("utf-8").decode(bytes);
  return { text, notUtf8: text.includes("\uFFFD") };
}

/** A line ending a file uses. */
export type LineEnding = "\n" | "\r\n" | "\r";

/** The line ending of `text`: the first one found, or "\n" if there is
 *  none (a single-line file). */
export function detectLineEnding(text: string): LineEnding {
  const i = text.search(/\r\n|\r|\n/);
  if (i < 0) return "\n";
  if (text[i] === "\n") return "\n";
  return text[i + 1] === "\n" ? "\r\n" : "\r";
}

/**
 * The text to write for the editor's `text`, whose lines are joined by
 * "\n" (the editor reads any line ending), with the file's own `ending`,
 * so saving does not change a file's line endings.
 */
export function toFileText(text: string, ending: LineEnding): string {
  return ending === "\n" ? text : text.split("\n").join(ending);
}

/** The last part of a path, for titles and messages. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** What opening a file gave. Never fails silently: a file that cannot be
 *  read gives the reason, for the status line. */
export type OpenResult =
  | { readonly ok: true; readonly path: string; readonly text: string; readonly notUtf8: boolean }
  | { readonly ok: false; readonly path: string; readonly error: string };

/** Reads the file at `path` with `load` (the app's `load_file`) and decodes
 *  it. */
export async function openPath(path: string, load: (path: string) => Promise<ArrayBuffer>): Promise<OpenResult> {
  try {
    const { text, notUtf8 } = decodeProgram(await load(path));
    return { ok: true, path, text, notUtf8 };
  } catch (e) {
    return { ok: false, path, error: String(e) };
  }
}

/** The bytes to save for the editor's `text`: UTF-8, with the file's own
 *  line ending. */
export function fileBytes(text: string, ending: LineEnding): Uint8Array {
  return new TextEncoder().encode(toFileText(text, ending));
}
