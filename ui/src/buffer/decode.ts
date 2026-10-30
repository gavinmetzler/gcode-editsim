// Decodes the render buffer the core produces (ARCHITECTURE.md section 8):
// one little-endian byte block with a header, vertices, a segment table, a
// diagnostic table and a string pool. Offsets come from the generated
// constants, never written by hand here.
//
// Vertex positions and kinds are zero-copy typed-array views over the
// buffer, ready for GPU upload. Everything is checked before use: a buffer
// that is truncated or inconsistent gives a DecodeError, never an
// out-of-range read.

import {
  ALIGN,
  DIAG,
  HEADER,
  MAGIC,
  SEGMENT,
  VERSION,
  VERTEX_POSITION_SIZE,
} from "../generated/render_buffer";

/** The bytes are not a render buffer this decoder can read. */
export class DecodeError extends Error {
  override name = "DecodeError";
}

/** The header's counts, origin and section offsets. */
export interface Header {
  readonly version: number;
  readonly flags: number;
  readonly vertexCount: number;
  readonly segmentCount: number;
  readonly diagCount: number;
  readonly channelCount: number;
  /** Vertex positions are relative to this point (f64), in mm. */
  readonly origin: readonly [number, number, number];
  readonly verticesOffset: number;
  readonly segmentsOffset: number;
  readonly diagsOffset: number;
  readonly stringsOffset: number;
}

/** One segment table entry. */
export interface Segment {
  readonly firstVertex: number;
  readonly vertexCount: number;
  /** 1-based source line. */
  readonly srcLine: number;
  readonly srcFile: number;
  /** Low 16 bits of the 0-based block index. */
  readonly blockLow: number;
  readonly tool: number;
  /** A `SEG_KIND` value. */
  readonly kind: number;
  /** `SEG_FLAG` bits. */
  readonly flags: number;
  /** Feed rate, mm/min. */
  readonly feed: number;
}

/** One diagnostic. */
export interface Diagnostic {
  /** 1-based source line; 0 if not tied to a line. */
  readonly srcLine: number;
  readonly srcFile: number;
  /** A `SEVERITY` value. */
  readonly severity: number;
  /** A `DIAG_CODE` value. */
  readonly code: number;
  readonly message: string;
}

const VERTEX_KIND_SIZE = 1;

/** `true` on little-endian hosts, where typed arrays can view the buffer. */
export const HOST_LITTLE_ENDIAN = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

/** A decoded render buffer. */
export interface RenderBuffer {
  readonly header: Header;
  /** x, y, z per vertex, relative to `header.origin`. */
  readonly positions: Float32Array;
  /** One `SEG_KIND` per vertex. */
  readonly vertexKinds: Uint8Array;
  readonly diagnostics: readonly Diagnostic[];
  /** Segment `index` (0 to `header.segmentCount - 1`). */
  segment(index: number): Segment;
  /** The source line of segment `index`, without decoding the rest. */
  segmentLine(index: number): number;
}

function fail(message: string): never {
  throw new DecodeError(message);
}

/** Options for tests. */
export interface DecodeOptions {
  /** Pretend the host is big-endian, to exercise the copying path. */
  readonly forceCopy?: boolean;
}

/** Decodes `buffer`, checking every count and offset first. */
export function decodeRenderBuffer(buffer: ArrayBuffer, options: DecodeOptions = {}): RenderBuffer {
  const length = buffer.byteLength;
  if (length < HEADER.SIZE) fail(`buffer is ${String(length)} bytes, shorter than the header`);
  const view = new DataView(buffer);
  const u32 = (at: number): number => view.getUint32(at, true);

  const magic = u32(HEADER.MAGIC);
  if (magic !== MAGIC) fail(`not a render buffer (magic 0x${magic.toString(16)})`);
  const version = view.getUint16(HEADER.VERSION, true);
  if (version !== VERSION) fail(`render buffer version ${String(version)}, this app reads ${String(VERSION)}`);

  const header: Header = {
    version,
    flags: view.getUint16(HEADER.FLAGS, true),
    vertexCount: u32(HEADER.VERTEX_COUNT),
    segmentCount: u32(HEADER.SEGMENT_COUNT),
    diagCount: u32(HEADER.DIAG_COUNT),
    channelCount: u32(HEADER.CHANNEL_COUNT),
    origin: [
      view.getFloat64(HEADER.ORIGIN, true),
      view.getFloat64(HEADER.ORIGIN + 8, true),
      view.getFloat64(HEADER.ORIGIN + 16, true),
    ],
    verticesOffset: u32(HEADER.VERTICES_OFFSET),
    segmentsOffset: u32(HEADER.SEGMENTS_OFFSET),
    diagsOffset: u32(HEADER.DIAGS_OFFSET),
    stringsOffset: u32(HEADER.STRINGS_OFFSET),
  };

  // Sections in order, aligned, each large enough for its count.
  const sections: [string, number, number][] = [
    ["vertices", header.verticesOffset, header.vertexCount * (VERTEX_POSITION_SIZE + VERTEX_KIND_SIZE)],
    ["segments", header.segmentsOffset, header.segmentCount * SEGMENT.SIZE],
    ["diagnostics", header.diagsOffset, header.diagCount * DIAG.SIZE],
    ["strings", header.stringsOffset, 0],
  ];
  let end = HEADER.SIZE;
  for (const [name, offset, size] of sections) {
    if (offset % ALIGN !== 0) fail(`${name} section is not ${String(ALIGN)}-byte aligned`);
    if (offset < end) fail(`${name} section overlaps the one before it`);
    if (offset + size > length) fail(`${name} section runs past the end of the buffer`);
    end = offset + size;
  }

  const vc = header.vertexCount;
  const kindsOffset = header.verticesOffset + vc * VERTEX_POSITION_SIZE;
  const littleEndian = HOST_LITTLE_ENDIAN && options.forceCopy !== true;
  let positions: Float32Array;
  if (littleEndian) {
    positions = new Float32Array(buffer, header.verticesOffset, vc * 3);
  } else {
    positions = new Float32Array(vc * 3);
    for (let i = 0; i < vc * 3; i += 1) {
      positions[i] = view.getFloat32(header.verticesOffset + i * 4, true);
    }
  }
  const vertexKinds = new Uint8Array(buffer, kindsOffset, vc);

  const segAt = (index: number): number => {
    if (!Number.isInteger(index) || index < 0 || index >= header.segmentCount) {
      throw new RangeError(`segment ${String(index)} of ${String(header.segmentCount)}`);
    }
    return header.segmentsOffset + index * SEGMENT.SIZE;
  };
  const segment = (index: number): Segment => {
    const at = segAt(index);
    const fileAndBlock = u32(at + SEGMENT.SRC_FILE_AND_BLOCK);
    return {
      firstVertex: u32(at + SEGMENT.FIRST_VERTEX),
      vertexCount: u32(at + SEGMENT.VERTEX_COUNT),
      srcLine: u32(at + SEGMENT.SRC_LINE),
      srcFile: fileAndBlock >>> 16,
      blockLow: fileAndBlock & 0xffff,
      tool: view.getUint16(at + SEGMENT.TOOL, true),
      kind: view.getUint8(at + SEGMENT.KIND),
      flags: view.getUint8(at + SEGMENT.FLAGS),
      feed: view.getFloat32(at + SEGMENT.FEED, true),
    };
  };
  for (let i = 0; i < header.segmentCount; i += 1) {
    const s = segment(i);
    if (s.firstVertex + s.vertexCount > vc) fail(`segment ${String(i)} refers to vertices past the end`);
  }

  const decoder = new TextDecoder("utf-8", { fatal: true });
  const strings = new Uint8Array(buffer, header.stringsOffset, length - header.stringsOffset);
  const diagnostics: Diagnostic[] = [];
  for (let i = 0; i < header.diagCount; i += 1) {
    const at = header.diagsOffset + i * DIAG.SIZE;
    const offset = u32(at + DIAG.MESSAGE_OFFSET);
    const size = u32(at + DIAG.MESSAGE_LENGTH);
    if (offset + size > strings.length) fail(`diagnostic ${String(i)} message runs past the end`);
    let message: string;
    try {
      message = decoder.decode(strings.subarray(offset, offset + size));
    } catch {
      fail(`diagnostic ${String(i)} message is not UTF-8`);
    }
    diagnostics.push({
      srcLine: u32(at + DIAG.SRC_LINE),
      srcFile: view.getUint16(at + DIAG.SRC_FILE, true),
      severity: view.getUint8(at + DIAG.SEVERITY),
      code: view.getUint16(at + DIAG.CODE, true),
      message,
    });
  }

  return {
    header,
    positions,
    vertexKinds,
    diagnostics,
    segment,
    segmentLine: (index: number) => u32(segAt(index) + SEGMENT.SRC_LINE),
  };
}
