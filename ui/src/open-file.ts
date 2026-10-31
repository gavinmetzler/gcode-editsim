// Reading a program file the user chose. Never fails silently: a file that
// cannot be read gives a result with the reason, for the status line.

/** What reading a chosen file gave. */
export type OpenResult =
  | { readonly ok: true; readonly name: string; readonly text: string; readonly notUtf8: boolean }
  | { readonly ok: false; readonly name: string; readonly error: string };

/** Reads `file` as UTF-8 text; bytes that are not UTF-8 become U+FFFD. */
export async function readProgramFile(file: File): Promise<OpenResult> {
  try {
    const text = new TextDecoder("utf-8").decode(await file.arrayBuffer());
    return { ok: true, name: file.name, text, notUtf8: text.includes("�") };
  } catch (e) {
    return { ok: false, name: file.name, error: String(e) };
  }
}
