import { describe, expect, it } from "vitest";

import { readProgramFile } from "../src/open-file";

describe("readProgramFile", () => {
  it("reads UTF-8 text", async () => {
    const result = await readProgramFile(new File(["G0 X1.\n"], "a.nc"));
    expect(result).toEqual({ ok: true, name: "a.nc", text: "G0 X1.\n", notUtf8: false });
  });

  it("notes text that is not UTF-8", async () => {
    const result = await readProgramFile(new File([new Uint8Array([0x47, 0x30, 0xff])], "b.nc"));
    expect(result.ok && result.notUtf8).toBe(true);
  });

  it("reports a file that cannot be read instead of failing silently", async () => {
    // A file whose contents cannot be read, as when the sandbox's copy of
    // the chosen file is gone.
    const broken = {
      name: "gone.nc",
      arrayBuffer: () => Promise.reject(new Error("No such file")),
    } as unknown as File;
    const result = await readProgramFile(broken);
    expect(result).toEqual({ ok: false, name: "gone.nc", error: "Error: No such file" });
  });
});
