//! Golden-file helpers for tests.
//!
//! [`summary`] renders an IR [`Program`] as compact, human-reviewable JSON:
//! one entry per segment showing only the values that matter for review
//! (end point by axis letter, arc centre and radius, feed, tool, flags),
//! rounded to 1e-6 so rounding noise does not cause churn. [`compare`]
//! checks a result against an expected file's contents.
//!
//! This crate does no file access itself (core crates never do); tests read
//! and write the files.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use ir::{DiagCode, Program, SegFlags, SegKind, Severity};
use serde::Serialize;

/// Environment variable that, when set, tells golden tests to rewrite
/// expected files instead of comparing against them.
pub const UPDATE_ENV: &str = "UPDATE_GOLDEN";

#[derive(Serialize)]
struct Summary {
    segments: Vec<SegmentSummary>,
    diagnostics: Vec<DiagSummary>,
    tools_used: Vec<u32>,
    bbox: Option<BboxSummary>,
}

#[derive(Serialize)]
struct SegmentSummary {
    line: u32,
    kind: SegKind,
    end: BTreeMap<char, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    center: Option<BTreeMap<char, f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    radius: Option<f64>,
    #[serde(skip_serializing_if = "is_zero")]
    feed: f64,
    #[serde(skip_serializing_if = "is_zero")]
    rpm: f64,
    tool: u32,
    work_offset: u16,
    #[serde(skip_serializing_if = "no_flags")]
    flags: SegFlags,
}

#[derive(Serialize)]
struct DiagSummary {
    line: u32,
    severity: Severity,
    code: DiagCode,
    message: String,
}

#[derive(Serialize)]
struct BboxSummary {
    min: BTreeMap<char, f64>,
    max: BTreeMap<char, f64>,
}

fn no_flags(f: &SegFlags) -> bool {
    f.is_empty()
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn round(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // Avoid "-0.0" in the output.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn by_letter(values: &[f64], letters: &[char]) -> BTreeMap<char, f64> {
    letters
        .iter()
        .zip(values)
        .map(|(l, v)| (*l, round(*v)))
        .collect()
}

/// Renders `program` as pretty-printed JSON for golden files.
///
/// `axis_letters` names the machine's axes in order; only those axes are
/// shown, by letter.
pub fn summary(program: &Program, axis_letters: &[char]) -> String {
    let segments = program
        .channels
        .iter()
        .flat_map(|c| &c.segments)
        .map(|s| SegmentSummary {
            line: s.src.line,
            kind: s.kind,
            end: by_letter(&s.end, axis_letters),
            center: s.arc.as_ref().map(|a| {
                let plane = [usize::from(a.plane.first), usize::from(a.plane.second)];
                plane
                    .iter()
                    .filter_map(|&i| Some((*axis_letters.get(i)?, round(*a.center.get(i)?))))
                    .collect()
            }),
            radius: s.arc.as_ref().map(|a| round(a.radius)),
            feed: round(s.feed),
            rpm: round(s.spindle_rpm),
            tool: s.tool,
            work_offset: s.work_offset.0,
            flags: s.flags,
        })
        .collect();
    let diagnostics = program
        .diagnostics
        .iter()
        .map(|d| DiagSummary {
            line: d.src.line,
            severity: d.severity,
            code: d.code,
            message: d.message.clone(),
        })
        .collect();
    let summary = Summary {
        segments,
        diagnostics,
        tools_used: program.meta.tools_used.clone(),
        bbox: program.meta.bbox.map(|b| BboxSummary {
            min: by_letter(&b.min, axis_letters),
            max: by_letter(&b.max, axis_letters),
        }),
    };
    // Serialising plain data with string/char keys cannot fail.
    let mut text = serde_json::to_string_pretty(&summary).unwrap_or_default();
    text.push('\n');
    text
}

/// Compares `actual` with the expected file's contents (`None` if the file
/// does not exist). Line endings are normalised. Returns a readable
/// explanation on mismatch.
pub fn compare(name: &str, expected: Option<&str>, actual: &str) -> Result<(), String> {
    match expected {
        None => Err(format!(
            "{name}: no expected output yet. Run with {UPDATE_ENV}=1, review the new file, and \
             commit it.\n{actual}"
        )),
        Some(e) if e.replace("\r\n", "\n") == actual => Ok(()),
        Some(e) => {
            let first_diff = e
                .lines()
                .zip(actual.lines())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| e.lines().count().min(actual.lines().count()));
            Err(format!(
                "{name}: output differs from the expected file, first at line {}.\n\
                 If the change is intended, run with {UPDATE_ENV}=1 and review the diff.\n\
                 --- expected\n{e}\n--- actual\n{actual}",
                first_diff + 1
            ))
        }
    }
}

/// `true` if golden tests should rewrite expected files.
pub fn update_requested() -> bool {
    std::env::var_os(UPDATE_ENV).is_some()
}
