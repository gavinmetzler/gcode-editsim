//! The render buffer: the binary form in which a program crosses to the UI
//! (ARCHITECTURE.md §8).
//!
//! One contiguous little-endian byte block holding a header, the vertices
//! (struct-of-arrays, ready for GPU upload), a segment table, a diagnostic
//! table and a UTF-8 string pool. Every section starts on a 4-byte
//! boundary. Vertex positions are `f32` relative to an `f64` origin, the
//! centre of the vertices' bounding box, so precision is not lost far from
//! machine zero.
//!
//! The layout is defined once, in [`layout`]. Changing it means bumping
//! [`layout::VERSION`] and updating the shared fixture
//! `tests/golden/render_buffer_v2.bin` (and, from M2.4, the generated
//! TypeScript constants).

use crate::diag::Diagnostic;
use crate::geom::{tessellate, tessellation_len, Pos};
use crate::segment::{Program, Segment};

/// Byte offsets and sizes of the render buffer, version 2.
///
/// All offsets are in bytes; header fields are from the start of the
/// buffer, table fields from the start of their entry. All integers and
/// floats are little-endian.
pub mod layout {
    /// `magic` value: 0x47435052, "GCPR" read as a big-endian number. In
    /// the (little-endian) buffer the bytes are `52 50 43 47` ("RPCG").
    pub const MAGIC: u32 = 0x4743_5052;
    /// Layout version this module describes.
    /// Version 2 (M3.8) added each segment's estimated time.
    pub const VERSION: u16 = 2;
    /// Header `flags` bit: the buffer is one chunk of a larger result and
    /// more follow. Reserved for chunked results (§8.5); never set yet.
    pub const FLAG_PARTIAL: u16 = 1 << 0;
    /// Every section starts on a multiple of this, and the whole buffer's
    /// length is one.
    pub const ALIGN: usize = 4;

    /// Header size.
    pub const HEADER_SIZE: usize = 64;
    /// `u32` magic.
    pub const HEADER_MAGIC: usize = 0;
    /// `u16` layout version.
    pub const HEADER_VERSION: usize = 4;
    /// `u16` flags.
    pub const HEADER_FLAGS: usize = 6;
    /// `u32` number of vertices.
    pub const HEADER_VERTEX_COUNT: usize = 8;
    /// `u32` number of segment table entries.
    pub const HEADER_SEGMENT_COUNT: usize = 12;
    /// `u32` number of diagnostic table entries.
    pub const HEADER_DIAG_COUNT: usize = 16;
    /// `u32` number of channels.
    pub const HEADER_CHANNEL_COUNT: usize = 20;
    /// `f64` × 3 origin that vertex positions are relative to.
    pub const HEADER_ORIGIN: usize = 24;
    /// `u32` offset of the vertices section.
    pub const HEADER_VERTICES_OFFSET: usize = 48;
    /// `u32` offset of the segment table.
    pub const HEADER_SEGMENTS_OFFSET: usize = 52;
    /// `u32` offset of the diagnostic table.
    pub const HEADER_DIAGS_OFFSET: usize = 56;
    /// `u32` offset of the string pool.
    pub const HEADER_STRINGS_OFFSET: usize = 60;

    /// Bytes per vertex position (`f32` × 3). The positions come first in
    /// the vertices section, followed by one `u8` kind per vertex
    /// ([`crate::SegKind`] of the vertex's segment), then padding.
    pub const VERTEX_POSITION_SIZE: usize = 12;

    /// Segment table entry size.
    pub const SEGMENT_SIZE: usize = 28;
    /// `u32` index of the segment's first vertex.
    pub const SEGMENT_FIRST_VERTEX: usize = 0;
    /// `u32` number of vertices (2 for a straight move, more for an arc, 1
    /// for an event).
    pub const SEGMENT_VERTEX_COUNT: usize = 4;
    /// `u32` 1-based source line.
    pub const SEGMENT_SRC_LINE: usize = 8;
    /// `u32`: source file index in the high 16 bits, the low 16 bits of the
    /// block index in the low 16 bits.
    pub const SEGMENT_SRC_FILE_AND_BLOCK: usize = 12;
    /// `u16` tool number (larger numbers are stored as 65535).
    pub const SEGMENT_TOOL: usize = 16;
    /// `u8` [`crate::SegKind`].
    pub const SEGMENT_KIND: usize = 18;
    /// `u8` low byte of [`crate::SegFlags`].
    pub const SEGMENT_FLAGS: usize = 19;
    /// `f32` feed rate, mm/min.
    pub const SEGMENT_FEED: usize = 20;
    /// `f32` estimated time, seconds ([`crate::Segment::time`]); NaN if
    /// unknown.
    pub const SEGMENT_TIME: usize = 24;

    /// Diagnostic table entry size.
    pub const DIAG_SIZE: usize = 20;
    /// `u32` 1-based source line (0: not tied to a line).
    pub const DIAG_SRC_LINE: usize = 0;
    /// `u16` source file index.
    pub const DIAG_SRC_FILE: usize = 4;
    /// `u8` severity ([`crate::Severity::id`]).
    pub const DIAG_SEVERITY: usize = 6;
    /// `u16` [`crate::DiagCode`] id.
    pub const DIAG_CODE: usize = 8;
    /// `u32` offset of the message within the string pool.
    pub const DIAG_MESSAGE_OFFSET: usize = 12;
    /// `u32` message length in bytes (UTF-8).
    pub const DIAG_MESSAGE_LENGTH: usize = 16;
}

/// A sensible default arc tolerance in mm for drawing (chord error).
pub const DEFAULT_TOLERANCE: f64 = 0.01;

/// Why a program could not be encoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    /// A count or offset does not fit the format's 32-bit fields.
    #[error("the program is too large for the render buffer: too many {0}")]
    TooLarge(&'static str),
}

/// The most vertices the app draws for one program (10 million, about
/// 130 MB of render buffer). A million-block program needs about 2
/// million; only very large arcs at a fine tolerance go past this.
pub const MAX_VERTICES: usize = 10_000_000;

/// How many vertices [`encode_render_buffer`] writes for `program` at
/// `tolerance` (saturating).
pub fn vertex_count(program: &Program, tolerance: f64) -> usize {
    program
        .channels
        .iter()
        .flat_map(|c| &c.segments)
        .fold(0usize, |n, seg| {
            n.saturating_add(tessellation_len(seg, tolerance))
        })
}

/// The arc tolerance to draw `program` with: `tolerance` if the drawing
/// fits in `max_vertices`, otherwise the finest tolerance found (doubling
/// each time) that fits. If nothing fits, the coarsest drawing (every arc
/// at its minimum of one point per 90 degrees). A crafted program of huge
/// arcs would otherwise need more memory than the machine has.
pub fn fit_tolerance(program: &Program, tolerance: f64, max_vertices: usize) -> f64 {
    let coarsest = vertex_count(program, f64::INFINITY);
    let mut tol = tolerance;
    let mut count = vertex_count(program, tol);
    // Doubling 200 times takes any sensible tolerance past every arc.
    for _ in 0..200 {
        // `!(tol > 0.0)` also stops on NaN: nothing to scale.
        if count <= max_vertices || count <= coarsest || !(tol > 0.0 && (tol * 2.0).is_finite()) {
            break;
        }
        tol *= 2.0;
        count = vertex_count(program, tol);
    }
    tol
}

/// Encodes `program` as a render buffer, layout version 2.
///
/// - `tolerance` is the arc chord error in mm (see [`tessellate`]).
/// - `to_xyz` turns a machine position into the Cartesian point to draw,
///   e.g. the tool tip from the machine's forward kinematics. The IR does
///   not know which axis is which; the caller does.
///
/// Segments of all channels are written in channel order. Version 2 has
/// no per-segment channel number, so it can only describe single-channel
/// programs unambiguously.
pub fn encode_render_buffer<F>(
    program: &Program,
    tolerance: f64,
    to_xyz: F,
) -> Result<Vec<u8>, EncodeError>
where
    F: Fn(&Pos) -> [f64; 3],
{
    let segments: Vec<&Segment> = program.channels.iter().flat_map(|c| &c.segments).collect();

    // Vertices in f64 first: the origin depends on all of them.
    let mut points: Vec<[f64; 3]> = Vec::new();
    let mut kinds: Vec<u8> = Vec::new();
    let mut ranges: Vec<(u32, u32)> = Vec::with_capacity(segments.len());
    for seg in &segments {
        let first = points.len();
        for p in tessellate(seg, tolerance) {
            points.push(to_xyz(&p));
            kinds.push(seg.kind as u8);
        }
        ranges.push((
            to_u32(first, "vertices")?,
            to_u32(points.len() - first, "vertices")?,
        ));
    }
    let origin = bbox_centre(&points);

    let vertex_count = to_u32(points.len(), "vertices")?;
    let segment_count = to_u32(segments.len(), "segments")?;
    let diag_count = to_u32(program.diagnostics.len(), "diagnostics")?;
    let channel_count = to_u32(program.channels.len(), "channels")?;

    let vertices_offset = layout::HEADER_SIZE;
    let vertices_len = points.len() * (layout::VERTEX_POSITION_SIZE + 1);
    let segments_offset = aligned(vertices_offset + vertices_len);
    let diags_offset = segments_offset + segments.len() * layout::SEGMENT_SIZE;
    let strings_offset = diags_offset + program.diagnostics.len() * layout::DIAG_SIZE;
    let strings_len: usize = program.diagnostics.iter().map(|d| d.message.len()).sum();
    let total = aligned(strings_offset + strings_len);
    to_u32(total, "bytes")?;

    let mut out = Vec::with_capacity(total);

    // Header.
    put_u32(&mut out, layout::MAGIC);
    put_u16(&mut out, layout::VERSION);
    put_u16(&mut out, 0);
    put_u32(&mut out, vertex_count);
    put_u32(&mut out, segment_count);
    put_u32(&mut out, diag_count);
    put_u32(&mut out, channel_count);
    for v in origin {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for offset in [
        vertices_offset,
        segments_offset,
        diags_offset,
        strings_offset,
    ] {
        put_u32(&mut out, to_u32(offset, "bytes")?);
    }

    // Vertices: positions, then kinds, then padding.
    for p in &points {
        for (v, o) in p.iter().zip(origin) {
            // Narrowing to f32 is the point of the origin.
            out.extend_from_slice(&((v - o) as f32).to_le_bytes());
        }
    }
    out.extend_from_slice(&kinds);
    pad(&mut out);

    // Segment table.
    for (seg, (first, count)) in segments.iter().zip(ranges) {
        put_u32(&mut out, first);
        put_u32(&mut out, count);
        put_u32(&mut out, seg.src.line);
        put_u32(
            &mut out,
            (u32::from(seg.src.file) << 16) | (seg.src.block & 0xFFFF),
        );
        put_u16(&mut out, u16::try_from(seg.tool).unwrap_or(u16::MAX));
        out.push(seg.kind as u8);
        out.push(seg.flags.0.to_le_bytes()[0]);
        out.extend_from_slice(&(seg.feed as f32).to_le_bytes());
        let time = seg.time.map_or(f32::NAN, |t| t as f32);
        out.extend_from_slice(&time.to_le_bytes());
    }

    // Diagnostic table, then the string pool.
    let mut message_offset = 0usize;
    for d in &program.diagnostics {
        put_diag(&mut out, d, to_u32(message_offset, "bytes")?)?;
        message_offset += d.message.len();
    }
    for d in &program.diagnostics {
        out.extend_from_slice(d.message.as_bytes());
    }
    pad(&mut out);

    debug_assert_eq!(out.len(), total);
    Ok(out)
}

fn put_diag(out: &mut Vec<u8>, d: &Diagnostic, message_offset: u32) -> Result<(), EncodeError> {
    put_u32(out, d.src.line);
    put_u16(out, d.src.file);
    out.push(d.severity.id());
    out.push(0);
    put_u16(out, d.code.id());
    put_u16(out, 0);
    put_u32(out, message_offset);
    put_u32(out, to_u32(d.message.len(), "bytes")?);
    Ok(())
}

/// Centre of the bounding box of the finite points; zero if there are none.
fn bbox_centre(points: &[[f64; 3]]) -> [f64; 3] {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in points {
        if p.iter().all(|v| v.is_finite()) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    let mut c = [0.0; 3];
    for i in 0..3 {
        if lo[i] <= hi[i] {
            c[i] = lo[i] + (hi[i] - lo[i]) / 2.0;
        }
    }
    c
}

fn to_u32(n: usize, what: &'static str) -> Result<u32, EncodeError> {
    u32::try_from(n).map_err(|_| EncodeError::TooLarge(what))
}

fn aligned(n: usize) -> usize {
    n.div_ceil(layout::ALIGN) * layout::ALIGN
}

fn pad(out: &mut Vec<u8>) {
    out.resize(aligned(out.len()), 0);
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
