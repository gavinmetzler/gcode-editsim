//! Tests for the render buffer encoder, including the shared fixture
//! `tests/golden/render_buffer_v2.bin` that the TypeScript decoder checks
//! as well (ARCHITECTURE.md §8.4).
//!
//! The decoder here is written independently of the encoder (plain byte
//! offsets from §8.3), so a mistake in `ir::render::layout` shows up.

mod common;

use std::path::PathBuf;

use common::{arc, pos, seg};
use ir::render::layout;
use ir::{
    encode_render_buffer, ChannelStream, DiagCode, Diagnostic, Pos, Program, SegFlags, SegKind,
    Segment, SourceRef,
};

// ---- a minimal independent decoder ----------------------------------------

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn f64_at(b: &[u8], at: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    f64::from_le_bytes(a)
}

struct Header {
    magic: u32,
    version: u16,
    flags: u16,
    vertex_count: u32,
    segment_count: u32,
    diag_count: u32,
    channel_count: u32,
    origin: [f64; 3],
    vertices_offset: u32,
    segments_offset: u32,
    diags_offset: u32,
    strings_offset: u32,
}

fn header(b: &[u8]) -> Header {
    Header {
        magic: u32_at(b, 0),
        version: u16_at(b, 4),
        flags: u16_at(b, 6),
        vertex_count: u32_at(b, 8),
        segment_count: u32_at(b, 12),
        diag_count: u32_at(b, 16),
        channel_count: u32_at(b, 20),
        origin: [f64_at(b, 24), f64_at(b, 32), f64_at(b, 40)],
        vertices_offset: u32_at(b, 48),
        segments_offset: u32_at(b, 52),
        diags_offset: u32_at(b, 56),
        strings_offset: u32_at(b, 60),
    }
}

fn vertex(b: &[u8], h: &Header, i: usize) -> [f32; 3] {
    let at = h.vertices_offset as usize + i * 12;
    [f32_at(b, at), f32_at(b, at + 4), f32_at(b, at + 8)]
}

fn vertex_kind(b: &[u8], h: &Header, i: usize) -> u8 {
    b[h.vertices_offset as usize + h.vertex_count as usize * 12 + i]
}

#[derive(Debug, PartialEq)]
struct SegEntry {
    first_vertex: u32,
    vertex_count: u32,
    src_line: u32,
    src_file_and_block: u32,
    tool: u16,
    kind: u8,
    flags: u8,
    feed: f32,
    /// `None` for NaN (time unknown).
    time: Option<f32>,
}

fn seg_entry(b: &[u8], h: &Header, i: usize) -> SegEntry {
    let at = h.segments_offset as usize + i * 28;
    let time = f32_at(b, at + 24);
    SegEntry {
        first_vertex: u32_at(b, at),
        vertex_count: u32_at(b, at + 4),
        src_line: u32_at(b, at + 8),
        src_file_and_block: u32_at(b, at + 12),
        tool: u16_at(b, at + 16),
        kind: b[at + 18],
        flags: b[at + 19],
        feed: f32_at(b, at + 20),
        time: (!time.is_nan()).then_some(time),
    }
}

#[derive(Debug, PartialEq)]
struct DiagEntry {
    src_line: u32,
    src_file: u16,
    severity: u8,
    code: u16,
    message: String,
}

fn diag_entry(b: &[u8], h: &Header, i: usize) -> DiagEntry {
    let at = h.diags_offset as usize + i * 20;
    let off = h.strings_offset as usize + u32_at(b, at + 12) as usize;
    let len = u32_at(b, at + 16) as usize;
    assert_eq!(b[at + 7], 0, "reserved");
    assert_eq!(u16_at(b, at + 10), 0, "reserved");
    DiagEntry {
        src_line: u32_at(b, at),
        src_file: u16_at(b, at + 4),
        severity: b[at + 6],
        code: u16_at(b, at + 8),
        message: String::from_utf8(b[off..off + len].to_vec()).expect("utf-8"),
    }
}

// ---- helpers ---------------------------------------------------------------

/// The first three axes as x, y, z (a 3-axis mill's kinematics).
fn xyz(p: &Pos) -> [f64; 3] {
    [p[0], p[1], p[2]]
}

fn program(segments: Vec<Segment>, diagnostics: Vec<Diagnostic>) -> Program {
    Program {
        channels: vec![ChannelStream {
            channel: 0,
            segments,
        }],
        diagnostics,
        files: vec!["test.nc".to_owned()],
        ..Program::default()
    }
}

fn on(mut s: Segment, line: u32, block: u32) -> Segment {
    s.src.line = line;
    s.src.block = block;
    s.tool = 1;
    s
}

fn encode(p: &Program, tol: f64) -> Vec<u8> {
    encode_render_buffer(p, tol, xyz).expect("encodes")
}

// ---- the shared fixture ----------------------------------------------------

/// The fixture program. Every value checked below is worked out by hand
/// from this, and the TypeScript test checks the same values.
fn fixture_program() -> Program {
    let p0 = pos(&[1000.0, 2000.0, 50.0]);
    let p1 = pos(&[1010.0, 2000.0, 5.0]);
    let p2 = pos(&[1010.0, 2000.0, -1.0]);
    let p3 = pos(&[1000.0, 2010.0, -1.0]);

    let mut rapid = on(seg(SegKind::Rapid, p0, p1, 1), 3, 2);
    rapid.time = Some(0.125);
    let mut spindle = on(seg(SegKind::SpindleCw, p1, p1, 1), 4, 3);
    spindle.time = Some(0.0);
    let mut feed = on(seg(SegKind::Linear, p1, p2, 1), 5, 4);
    feed.feed = 250.0;
    feed.time = Some(1.44);
    feed.flags = SegFlags::OUT_OF_LIMITS;
    // Quarter circle, centre (1000, 2000), radius 10, counter-clockwise.
    let mut quarter = on(
        arc(
            SegKind::ArcCcw,
            &[1010.0, 2000.0, -1.0],
            &[1000.0, 2010.0, -1.0],
            &[1000.0, 2000.0],
            10.0,
        ),
        6,
        5,
    );
    quarter.feed = 500.0;
    quarter.time = Some(1.875);
    // Block index above 65535: only its low 16 bits (6) are stored. Its
    // time is unknown (NaN in the buffer).
    let mut end = on(seg(SegKind::ProgramEnd, p3, p3, 1), 7, 65_536 + 6);
    end.flags = SegFlags::POSITION_UNKNOWN;

    let diagnostics = vec![
        Diagnostic::warning(
            DiagCode::OutOfLimits,
            "move goes outside the travel limits",
            SourceRef {
                line: 5,
                ..SourceRef::default()
            },
        ),
        // Not tied to a line; non-ASCII to check byte (not char) lengths.
        Diagnostic::info(
            DiagCode::NotYetSimulated,
            "Ø10 – note",
            SourceRef::default(),
        ),
        Diagnostic::error(
            DiagCode::InvalidArc,
            "arc",
            SourceRef {
                line: 6,
                ..SourceRef::default()
            },
        ),
    ];
    program(vec![rapid, spindle, feed, quarter, end], diagnostics)
}

/// Arc tolerance for the fixture: coarse, so the quarter circle has
/// exactly two chords (2·acos(0.9) ≈ 51.7° per step; 90° needs 2).
const FIXTURE_TOLERANCE: f64 = 1.0;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/render_buffer_v2.bin")
}

#[test]
fn fixture_matches_the_committed_file() {
    let bytes = encode(&fixture_program(), FIXTURE_TOLERANCE);
    let path = fixture_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &bytes).expect("write fixture");
    }
    let committed = std::fs::read(&path).expect("fixture exists (UPDATE_GOLDEN=1 to create)");
    assert!(
        committed == bytes,
        "render buffer differs from {}; a layout change must bump the version",
        path.display()
    );
}

/// Decodes the committed fixture and checks every value by hand.
#[test]
fn fixture_decodes_to_the_expected_values() {
    let b = std::fs::read(fixture_path()).expect("fixture exists");
    let h = header(&b);

    assert_eq!(&b[0..4], b"RPCG");
    assert_eq!(h.magic, 0x4743_5052);
    assert_eq!(h.version, 2);
    assert_eq!(h.flags, 0);
    assert_eq!(h.vertex_count, 9);
    assert_eq!(h.segment_count, 5);
    assert_eq!(h.diag_count, 3);
    assert_eq!(h.channel_count, 1);
    // Centre of x 1000..1010, y 2000..2010, z -1..50.
    assert_eq!(h.origin, [1005.0, 2005.0, 24.5]);
    // 64-byte header; 9 × 12 position bytes + 9 kind bytes = 117, padded
    // to 120; 5 × 28 segment bytes; 3 × 20 diagnostic bytes; 51 string
    // bytes padded to the end.
    assert_eq!(h.vertices_offset, 64);
    assert_eq!(h.segments_offset, 184);
    assert_eq!(h.diags_offset, 324);
    assert_eq!(h.strings_offset, 384);
    assert_eq!(b.len(), 436);

    // Vertices relative to the origin. The arc's middle point is at 45°:
    // 10·cos 45° = 7.0710678, i.e. 2.0710678 from the origin.
    let m = 10.0 * std::f64::consts::FRAC_1_SQRT_2 - 5.0;
    let expected: [[f64; 3]; 9] = [
        [-5.0, -5.0, 25.5], // rapid start
        [5.0, -5.0, -19.5], // rapid end
        [5.0, -5.0, -19.5], // spindle event
        [5.0, -5.0, -19.5], // feed start
        [5.0, -5.0, -25.5], // feed end
        [5.0, -5.0, -25.5], // arc start
        [m, m, -25.5],      // arc middle
        [-5.0, 5.0, -25.5], // arc end
        [-5.0, 5.0, -25.5], // program end event
    ];
    for (i, e) in expected.iter().enumerate() {
        let v = vertex(&b, &h, i);
        for k in 0..3 {
            assert!(
                (f64::from(v[k]) - e[k]).abs() < 1e-6,
                "vertex {i}: {v:?} vs {e:?}"
            );
        }
    }
    let kinds: Vec<u8> = (0..9).map(|i| vertex_kind(&b, &h, i)).collect();
    assert_eq!(kinds, vec![0, 0, 9, 1, 1, 3, 3, 3, 15]);
    // Padding after the kinds is zero.
    assert_eq!(&b[181..184], &[0, 0, 0]);

    let entry = |first_vertex, vertex_count, src_line, block, kind, flags, feed, time| SegEntry {
        first_vertex,
        vertex_count,
        src_line,
        src_file_and_block: block,
        tool: 1,
        kind,
        flags,
        feed,
        time,
    };
    let segs: Vec<SegEntry> = (0..5).map(|i| seg_entry(&b, &h, i)).collect();
    assert_eq!(
        segs,
        vec![
            entry(0, 2, 3, 2, 0, 0, 0.0, Some(0.125)),
            entry(2, 1, 4, 3, 9, 0, 0.0, Some(0.0)),
            entry(3, 2, 5, 4, 1, 2, 250.0, Some(1.44)),
            entry(5, 3, 6, 5, 3, 0, 500.0, Some(1.875)),
            entry(8, 1, 7, 6, 15, 4, 0.0, None),
        ]
    );

    let diags: Vec<DiagEntry> = (0..3).map(|i| diag_entry(&b, &h, i)).collect();
    let d = |src_line, severity, code, message: &str| DiagEntry {
        src_line,
        src_file: 0,
        severity,
        code,
        message: message.to_owned(),
    };
    assert_eq!(
        diags,
        vec![
            d(5, 1, 11, "move goes outside the travel limits"),
            d(0, 0, 13, "Ø10 – note"),
            d(6, 2, 9, "arc"),
        ]
    );
    // Message offsets and byte lengths: 35, then 13 ("Ø" and "–" take 2
    // and 3 bytes), then 3.
    assert_eq!(u32_at(&b, 324 + 12), 0);
    assert_eq!(u32_at(&b, 344 + 12), 35);
    assert_eq!(u32_at(&b, 344 + 16), 13);
    assert_eq!(u32_at(&b, 364 + 12), 48);
    assert_eq!(&b[435..436], &[0]);
}

// ---- other encoder behaviour -----------------------------------------------

#[test]
fn layout_constants_match_section_8_3() {
    assert_eq!(layout::MAGIC, 0x4743_5052);
    assert_eq!(layout::VERSION, 2);
    assert_eq!(layout::HEADER_SIZE, 64);
    assert_eq!(layout::HEADER_ORIGIN, 24);
    assert_eq!(layout::HEADER_STRINGS_OFFSET, 60);
    assert_eq!(layout::SEGMENT_SIZE, 28);
    assert_eq!(layout::SEGMENT_FEED, 20);
    assert_eq!(layout::SEGMENT_TIME + 4, layout::SEGMENT_SIZE);
    assert_eq!(layout::DIAG_SIZE, 20);
    assert_eq!(layout::DIAG_MESSAGE_LENGTH + 4, layout::DIAG_SIZE);
}

#[test]
fn empty_program_is_just_a_header() {
    let b = encode(&Program::default(), 0.01);
    let h = header(&b);
    assert_eq!(b.len(), 64);
    assert_eq!((h.vertex_count, h.segment_count, h.diag_count), (0, 0, 0));
    assert_eq!(h.channel_count, 0);
    assert_eq!(h.origin, [0.0; 3]);
    assert_eq!(
        [
            h.vertices_offset,
            h.segments_offset,
            h.diags_offset,
            h.strings_offset
        ],
        [64; 4]
    );
}

#[test]
fn sections_are_aligned_for_any_vertex_count() {
    for n in 0..9 {
        let p = pos(&[1.0, 2.0, 3.0]);
        let segs = (0..n)
            .map(|i| on(seg(SegKind::Dwell, p, p, 1), i + 1, i))
            .collect();
        let diags = vec![Diagnostic::info(
            DiagCode::NotYetSimulated,
            "x".repeat(n as usize),
            SourceRef::default(),
        )];
        let b = encode(&program(segs, diags), 0.01);
        let h = header(&b);
        assert_eq!(h.vertex_count, n);
        for off in [
            h.vertices_offset,
            h.segments_offset,
            h.diags_offset,
            h.strings_offset,
        ] {
            assert_eq!(off % 4, 0, "n = {n}");
        }
        assert_eq!(b.len() % 4, 0, "n = {n}");
        assert!(b.len() >= h.strings_offset as usize + n as usize);
    }
}

#[test]
fn far_from_zero_coordinates_keep_their_precision() {
    // ±10 000 mm with micrometre detail: as plain f32 these would be off by
    // up to ~0.5 µm; relative to the origin they are exact to ~1e-9.
    let a = pos(&[10_000.001, -10_000.002, 9_999.003]);
    let bpt = pos(&[10_000.004, -10_000.005, 9_999.006]);
    let p = program(vec![on(seg(SegKind::Linear, a, bpt, 1), 1, 0)], vec![]);
    let b = encode(&p, 0.01);
    let h = header(&b);
    for (i, want) in [a, bpt].iter().enumerate() {
        let v = vertex(&b, &h, i);
        for k in 0..3 {
            let got = h.origin[k] + f64::from(v[k]);
            assert!((got - want[k]).abs() < 1e-7, "vertex {i} axis {k}: {got}");
        }
    }
}

#[test]
fn the_caller_decides_which_axis_is_x_y_and_z() {
    let p = program(
        vec![on(
            seg(
                SegKind::Rapid,
                pos(&[1.0, 2.0, 3.0]),
                pos(&[1.0, 2.0, 3.0]),
                1,
            ),
            1,
            0,
        )],
        vec![],
    );
    // Swap: draw axis 2 as x and axis 0 as z.
    let b = encode_render_buffer(&p, 0.01, |q| [q[2], q[1], q[0]]).expect("encodes");
    assert_eq!(header(&b).origin, [3.0, 2.0, 1.0]);
}

#[test]
fn tolerance_controls_arc_vertices() {
    let quarter = arc(
        SegKind::ArcCw,
        &[0.0, 10.0],
        &[10.0, 0.0],
        &[0.0, 0.0],
        10.0,
    );
    let p = program(vec![quarter], vec![]);
    let coarse = header(&encode(&p, 1.0)).vertex_count;
    let fine = header(&encode(&p, 0.001)).vertex_count;
    assert_eq!(coarse, 3);
    assert!(fine > 30, "{fine}");
}

#[test]
fn large_tool_numbers_saturate_and_file_index_is_in_the_high_half() {
    let p0 = pos(&[]);
    let mut s = seg(SegKind::ToolChange, p0, p0, 1);
    s.tool = 70_000;
    s.src.file = 3;
    s.src.block = 0x1_2345;
    let b = encode(&program(vec![s], vec![]), 0.01);
    let e = seg_entry(&b, &header(&b), 0);
    assert_eq!(e.tool, u16::MAX);
    assert_eq!(e.src_file_and_block, (3 << 16) | 0x2345);
}

#[test]
fn channels_are_written_in_order() {
    let p0 = pos(&[]);
    let mut p = program(vec![on(seg(SegKind::Dwell, p0, p0, 1), 1, 0)], vec![]);
    p.channels.push(ChannelStream {
        channel: 1,
        segments: vec![on(seg(SegKind::Stop, p0, p0, 1), 9, 0)],
    });
    let b = encode(&p, 0.01);
    let h = header(&b);
    assert_eq!(h.channel_count, 2);
    assert_eq!(h.segment_count, 2);
    assert_eq!(seg_entry(&b, &h, 1).src_line, 9);
}

#[test]
fn only_the_low_byte_of_the_flags_is_stored() {
    let p0 = pos(&[]);
    let mut s = seg(SegKind::Dwell, p0, p0, 1);
    s.flags = SegFlags(0x0102);
    let b = encode(&program(vec![s], vec![]), 0.01);
    assert_eq!(seg_entry(&b, &header(&b), 0).flags, 0x02);
}

#[test]
fn non_finite_positions_do_not_break_the_origin() {
    let good = pos(&[2.0, 4.0, 6.0]);
    let bad = pos(&[f64::NAN, f64::INFINITY, 0.0]);
    let p = program(
        vec![
            on(seg(SegKind::Dwell, good, good, 1), 1, 0),
            on(seg(SegKind::Dwell, bad, bad, 1), 2, 1),
        ],
        vec![],
    );
    let b = encode(&p, 0.01);
    assert_eq!(header(&b).origin, [2.0, 4.0, 6.0]);
}
