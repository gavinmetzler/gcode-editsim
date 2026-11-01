// The Transform dialog's values as the request the app takes (M4.9).

import { describe, expect, it } from "vitest";

import { buildRequest, emptyForm, parseNumber } from "../src/transform/request";

describe("parseNumber", () => {
  it("reads plain numbers, and an empty field as the default", () => {
    expect(parseNumber("12.5", 0)).toBe(12.5);
    expect(parseNumber(" -3 ", 0)).toBe(-3);
    expect(parseNumber("+.5", 0)).toBe(0.5);
    expect(parseNumber("", 7)).toBe(7);
  });

  it("refuses anything else", () => {
    for (const bad of ["1,5", "abc", "1e3", "1.2.3", "--1", "NaN", "Infinity", "."]) {
      expect(parseNumber(bad, 0), bad).toBeNull();
    }
  });
});

describe("buildRequest", () => {
  it("translates by the offsets given", () => {
    const r = buildRequest(3, 7, { ...emptyForm(), kind: "translate", dx: "20", dz: "-1.5" });
    expect(r).toEqual({
      ok: true,
      request: { first: 3, last: 7, transform: { kind: "translate", offset: { X: 20, Y: 0, Z: -1.5 } } },
    });
  });

  it("rotates in the chosen plane about the centre", () => {
    const r = buildRequest(1, 2, { ...emptyForm(), kind: "rotate", plane: "ZX", angle: "90", cx: "5", cz: "1" });
    expect(r).toEqual({
      ok: true,
      request: {
        first: 1,
        last: 2,
        transform: { kind: "rotate", plane: ["Z", "X"], angle: 90, centre: { X: 5, Y: 0, Z: 1 } },
      },
    });
  });

  it("scales X and Y only", () => {
    const r = buildRequest(1, 1, { ...emptyForm(), kind: "scale", factor: "0.8", cx: "10", cy: "10", plane: "YZ" });
    expect(r).toEqual({
      ok: true,
      request: { first: 1, last: 1, transform: { kind: "scale", plane: ["X", "Y"], factor: 0.8, centre: { X: 10, Y: 10, Z: 0 } } },
    });
  });

  it("explains what is wrong with the values", () => {
    const bad = (form: Partial<ReturnType<typeof emptyForm>>): string => {
      const r = buildRequest(1, 1, { ...emptyForm(), ...form });
      return r.ok ? "" : r.error;
    };
    expect(bad({ kind: "translate", dx: "abc" })).toContain("dX");
    expect(bad({ kind: "translate" })).toContain("offset");
    expect(bad({ kind: "rotate", angle: "" })).toContain("angle");
    expect(bad({ kind: "rotate", angle: "0" })).toContain("angle");
    expect(bad({ kind: "scale", factor: "0" })).toContain("greater than zero");
    expect(bad({ kind: "scale", factor: "-2" })).toContain("greater than zero");
    expect(bad({ kind: "scale", factor: "1" })).toContain("1");
    expect(bad({ kind: "rotate", angle: "10", cy: "x" })).toContain("centre");
  });
});
