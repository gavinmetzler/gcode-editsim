//! Dialect- and machine-neutral IR: segments, diagnostics and render-buffer encoding.
//!
//! This is the central data model of the pipeline. Dialects and machines
//! produce it; renderers, statistics and simulation consume it. It depends
//! on no other workspace crate.
//!
//! All values use internal units: mm for lengths, degrees for angles,
//! mm/min for feed, rpm for spindle speed and seconds for time.
//!
//! With the `serde` feature enabled every type can be serialised, for
//! example to write golden test output as JSON.

#![forbid(unsafe_code)]

mod diag;
mod geom;
pub mod render;
mod segment;

pub use diag::{DiagCode, Diagnostic, Diagnostics, Severity};
pub use geom::{
    bounding_box, tessellate, ArcGeom, ArcPlane, BoundingBox, Pos, MAX_ARC_POINTS, MAX_AXES,
};
pub use render::{encode_render_buffer, EncodeError, DEFAULT_TOLERANCE};
pub use segment::{
    segments_for_line, ChannelStream, CycleStep, LengthUnit, Program, ProgramMeta, SegFlags,
    SegKind, Segment, SourceRef, WorkOffsetId,
};
