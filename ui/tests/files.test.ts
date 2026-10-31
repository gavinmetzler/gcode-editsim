import { describe, expect, it } from "vitest";

import { baseName, decodeProgram, detectLineEnding, fileBytes, openPath, toFileText } from "../src/files";

describe("decodeProgram", () => {
  it("decodes UTF-8 and notes bytes that are not", () => {
    expect(decodeProgram(new TextEncoder().encode("G0 X1.\n"))).toEqual({ text: "G0 X1.\n", notUtf8: false });
    expect(decodeProgram(new Uint8Array([0x47, 0x30, 0xff])).notUtf8).toBe(true);
  });
});

describe("line endings", () => {
  it("detects the first line ending", () => {
    expect(detectLineEnding("G0\r\nG1\r\n")).toBe("\r\n");
    expect(detectLineEnding("G0\nG1\n")).toBe("\n");
    expect(detectLineEnding("G0\rG1\r")).toBe("\r");
    expect(detectLineEnding("G0")).toBe("\n");
  });

  it("writes the editor's text back with the file's own line ending", () => {
    expect(toFileText("G0\nG1\n", "\r\n")).toBe("G0\r\nG1\r\n");
    expect(toFileText("G0\nG1", "\r")).toBe("G0\rG1");
    expect(toFileText("G0\nG1\n", "\n")).toBe("G0\nG1\n");
  });

  it("round-trips a CRLF file through the editor's \\n text", () => {
    const file = "%\r\nO1000\r\nG0 X1.\r\n%\r\n";
    const editorText = file.split(/\r\n|\r|\n/).join("\n");
    expect(toFileText(editorText, detectLineEnding(file))).toBe(file);
  });
});

describe("baseName", () => {
  it("takes the last part of Unix and Windows paths", () => {
    expect(baseName("/home/me/parts/a.nc")).toBe("a.nc");
    expect(baseName("C:\\parts\\b.nc")).toBe("b.nc");
    expect(baseName("c.nc")).toBe("c.nc");
  });
});

describe("openPath", () => {
  it("reads and decodes the file", async () => {
    const load = (p: string) => Promise.resolve(new TextEncoder().encode(`(${p})\n`).buffer);
    expect(await openPath("/a.nc", load)).toEqual({ ok: true, path: "/a.nc", text: "(/a.nc)\n", notUtf8: false });
  });

  it("notes text that is not UTF-8", async () => {
    const load = () => Promise.resolve(new Uint8Array([0x47, 0x30, 0xff]).buffer);
    const r = await openPath("/b.nc", load);
    expect(r.ok && r.notUtf8).toBe(true);
  });

  it("reports a file that cannot be read instead of failing silently", async () => {
    const load = () => Promise.reject(new Error("No such file"));
    expect(await openPath("/gone.nc", load)).toEqual({ ok: false, path: "/gone.nc", error: "Error: No such file" });
  });
});

describe("fileBytes", () => {
  it("encodes UTF-8 with the file's line ending", () => {
    expect(Array.from(fileBytes("Ü\nG0", "\r\n"))).toEqual([0xc3, 0x9c, 0x0d, 0x0a, 0x47, 0x30]);
  });
});
