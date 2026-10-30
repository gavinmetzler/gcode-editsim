// The cross-boundary test (ARCHITECTURE.md section 8.4): the TypeScript
// decoder reads the same fixture as core/ir/tests/render_buffer.rs and must
// find the same hand-worked values listed there.

import { describe, expect, it } from "vitest";

import { DecodeError, decodeRenderBuffer } from "../src/buffer/decode";
import { DIAG_CODE, HEADER, SEG_FLAG, SEG_KIND, SEVERITY } from "../src/generated/render_buffer";
import { fixture } from "./fixture";

describe("render_buffer_v1.bin", () => {
  const rb = decodeRenderBuffer(fixture());

  it("has the expected header", () => {
    expect(rb.header).toEqual({
      version: 1,
      flags: 0,
      vertexCount: 9,
      segmentCount: 5,
      diagCount: 3,
      channelCount: 1,
      // Centre of x 1000..1010, y 2000..2010, z -1..50.
      origin: [1005, 2005, 24.5],
      verticesOffset: 64,
      segmentsOffset: 184,
      diagsOffset: 304,
      stringsOffset: 364,
    });
  });

  it("has the expected vertices, relative to the origin", () => {
    // The arc's middle point is at 45 degrees: 10 cos 45 - 5.
    const m = 10 * Math.SQRT1_2 - 5;
    const expected = [
      [-5, -5, 25.5],
      [5, -5, -19.5],
      [5, -5, -19.5],
      [5, -5, -19.5],
      [5, -5, -25.5],
      [5, -5, -25.5],
      [m, m, -25.5],
      [-5, 5, -25.5],
      [-5, 5, -25.5],
    ];
    expected.forEach((e, i) => {
      for (let k = 0; k < 3; k += 1) {
        expect(rb.positions[i * 3 + k]).toBeCloseTo(e[k] ?? NaN, 5);
      }
    });
    expect(Array.from(rb.vertexKinds)).toEqual([
      SEG_KIND.Rapid,
      SEG_KIND.Rapid,
      SEG_KIND.SpindleCw,
      SEG_KIND.Linear,
      SEG_KIND.Linear,
      SEG_KIND.ArcCcw,
      SEG_KIND.ArcCcw,
      SEG_KIND.ArcCcw,
      SEG_KIND.ProgramEnd,
    ]);
  });

  it("has the expected segment table", () => {
    const entry = (
      firstVertex: number,
      vertexCount: number,
      srcLine: number,
      blockLow: number,
      kind: number,
      flags: number,
      feed: number,
    ) => ({ firstVertex, vertexCount, srcLine, srcFile: 0, blockLow, tool: 1, kind, flags, feed });
    expect([0, 1, 2, 3, 4].map((i) => rb.segment(i))).toEqual([
      entry(0, 2, 3, 2, SEG_KIND.Rapid, 0, 0),
      entry(2, 1, 4, 3, SEG_KIND.SpindleCw, 0, 0),
      entry(3, 2, 5, 4, SEG_KIND.Linear, SEG_FLAG.OUT_OF_LIMITS, 250),
      entry(5, 3, 6, 5, SEG_KIND.ArcCcw, 0, 500),
      // Block 65 542: only the low 16 bits (6) are stored.
      entry(8, 1, 7, 6, SEG_KIND.ProgramEnd, SEG_FLAG.POSITION_UNKNOWN, 0),
    ]);
    expect([0, 1, 2, 3, 4].map((i) => rb.segmentLine(i))).toEqual([3, 4, 5, 6, 7]);
  });

  it("has the expected diagnostics, with UTF-8 messages", () => {
    expect(rb.diagnostics).toEqual([
      { srcLine: 5, srcFile: 0, severity: SEVERITY.Warning, code: DIAG_CODE.OutOfLimits, message: "move goes outside the travel limits" },
      { srcLine: 0, srcFile: 0, severity: SEVERITY.Info, code: DIAG_CODE.NotYetSimulated, message: "Ø10 – note" },
      { srcLine: 6, srcFile: 0, severity: SEVERITY.Error, code: DIAG_CODE.InvalidArc, message: "arc" },
    ]);
  });

  it("views the positions without copying", () => {
    const buffer = fixture();
    const decoded = decodeRenderBuffer(buffer);
    expect(decoded.positions.buffer).toBe(buffer);
    expect(decoded.vertexKinds.buffer).toBe(buffer);
  });

  it("decodes the same values on the copying (big-endian host) path", () => {
    const copied = decodeRenderBuffer(fixture(), { forceCopy: true });
    expect(Array.from(copied.positions)).toEqual(Array.from(rb.positions));
    expect(copied.positions.buffer).not.toBe(rb.positions.buffer);
  });

  it("rejects segment indices out of range", () => {
    expect(() => rb.segment(5)).toThrow(RangeError);
    expect(() => rb.segment(-1)).toThrow(RangeError);
    expect(() => rb.segmentLine(1.5)).toThrow(RangeError);
  });
});

describe("bad buffers", () => {
  const patched = (at: number, value: number, size: 1 | 2 | 4 = 4): ArrayBuffer => {
    const buffer = fixture();
    const view = new DataView(buffer);
    if (size === 1) view.setUint8(at, value);
    else if (size === 2) view.setUint16(at, value, true);
    else view.setUint32(at, value, true);
    return buffer;
  };
  const rejects = (buffer: ArrayBuffer, text: string) => {
    expect(() => decodeRenderBuffer(buffer)).toThrow(DecodeError);
    expect(() => decodeRenderBuffer(buffer)).toThrow(text);
  };

  it("is too short for a header", () => {
    rejects(new ArrayBuffer(HEADER.SIZE - 1), "shorter than the header");
  });

  it("has the wrong magic or version", () => {
    rejects(patched(HEADER.MAGIC, 0x12345678), "not a render buffer");
    rejects(patched(HEADER.VERSION, 2, 2), "version 2");
  });

  it("has sections that are misaligned, overlapping or too long", () => {
    rejects(patched(HEADER.SEGMENTS_OFFSET, 186), "aligned");
    rejects(patched(HEADER.SEGMENTS_OFFSET, 100), "overlaps");
    rejects(patched(HEADER.SEGMENT_COUNT, 1000), "past the end");
    rejects(patched(HEADER.VERTEX_COUNT, 0xffffffff), "past the end");
  });

  it("has a segment pointing past the vertices", () => {
    rejects(patched(184 + 4 * 24 + 4, 99), "segment 4 refers to vertices past the end");
  });

  it("has a diagnostic message out of range or not UTF-8", () => {
    rejects(patched(304 + 16, 1000), "message runs past the end");
    rejects(patched(364 + 35, 0xff, 1), "not UTF-8");
  });

  it("never fails other than with a DecodeError on damaged input", () => {
    // Deterministic pseudo-random damage: flip bytes and truncate.
    let seed = 12345;
    const next = () => {
      seed = (seed * 1103515245 + 12345) >>> 0;
      return seed;
    };
    for (let round = 0; round < 2000; round += 1) {
      const source = new Uint8Array(fixture());
      const keep = round % 5 === 0 ? next() % source.length : source.length;
      const bytes = source.slice(0, keep);
      for (let k = 0; k < 1 + (round % 4); k += 1) {
        if (bytes.length > 0) bytes[next() % bytes.length] = next() & 0xff;
      }
      try {
        decodeRenderBuffer(bytes.buffer);
      } catch (e) {
        expect(e).toBeInstanceOf(DecodeError);
      }
    }
  });
});
