# ARCHITECTURE

Status: **draft v0.1**. Interfaces marked *provisional* are expected to change once a second dialect and a second machine type exist (see §12).

## 1. Goals

- A G-code simulator and backplotter in the spirit of CIMCOEdit / NCPlot: parse, interpret, visualize, analyze, and eventually simulate material removal.
- Runs as a fast desktop app on Linux, Windows and macOS, and as a usable app on Android and iOS, from one codebase.
- User-selectable **machine type** (mill, lathe, mill-turn, multi-channel, 5-axis) and **dialect** (Fanuc, Haas, Siemens, Heidenhain, GRBL, LinuxCNC, ...).
- **Initial implementation scope: 3-axis mill + Fanuc only.** Everything else is designed for, not built.

## 2. Non-goals (for now)

- Not a CAM system. No toolpath generation.
- Not a safety verification tool. Output must never be treated as proof a program is safe to run on a machine.
- No controller-exact emulation of every Fanuc option. We target the commonly used subset and report what we don't support.
- No cloud service or accounts. Everything runs on-device.

## 3. Principles

1. **Core is pure.** The `core/` crates have no UI, windowing, filesystem-assumption, or platform dependencies. They must compile to native and `wasm32`.
2. **Three separate concerns:** *dialect* (text → meaning), *machine* (physical axes/kinematics/tooling), *IR* (neutral motion output). None may leak into another.
3. **Data over code** where practical: dialect code tables, modal groups, and axis definitions live in data files; Rust code handles only genuinely irregular behaviour.
4. **Diagnostics are first-class output**, not logging. Every stage can emit warnings and errors tied to a source location.
5. **Binary at the boundary.** Bulk data crosses Rust → UI as typed arrays, never JSON.
6. **Everything is tested headlessly** with golden G-code files.

## 4. Pipeline

```
G-code text
  → gcode-syntax   : lexer + parser → Blocks (dialect-neutral AST)
  → dialect        : Blocks → Commands (machine-neutral intents), expands canned cycles,
                     tracks modal state, emits Diagnostics
  → interpreter    : applies Commands to a Machine model → Segments (IR), per channel
  → consumers      : backplot/render buffers, stats, cycle time, collision, material removal
```

## 5. Workspace layout

```
/
├─ ARCHITECTURE.md
├─ CLAUDE.md
├─ Cargo.toml                  # workspace
├─ core/
│  ├─ gcode-syntax/            # lexer, parser, expression AST. Knows no G-code semantics.
│  ├─ ir/                      # Segment types, Diagnostic, render-buffer encoding
│  ├─ dialect-api/             # Dialect trait, Command enum, ModalState, CodeTable loader
│  ├─ dialect-fanuc/           # first real dialect (+ data/fanuc.toml)
│  ├─ machine-api/             # Machine trait, AxisDef, Kinematics, Tool, Stock traits
│  ├─ machine-mill/            # first real machine (3-axis)
│  ├─ interpreter/             # glue: dialect + machine + state → IR
│  ├─ sim/                     # material removal, per machine family (mill first)
│  └─ testkit/                 # golden-file helpers, fixtures loader
├─ app/                        # Tauri 2 shell: thin command layer over core
├─ ui/                         # TypeScript: three.js viewport, CodeMirror editor
└─ tests/
   └─ golden/                  # *.nc input + expected .json / .bin output
```

Dependency rule (enforced in CI): arrows only point downward in the list above. `ir` depends on nothing in the workspace. `app` and `ui` may depend on `core`; `core` never depends on them.

## 6. Core concepts

### 6.1 Units and conventions (fixed, internal)

- Internal length unit: **millimetres**. Angles: **degrees**. Feed: **mm/min**. Spindle: **rpm**. Time: **seconds**.
- Dialect layer converts from G20/G21, diameter/radius programming, etc. at the boundary. Downstream code never sees inches.
- Coordinates are `f64` in core. Conversion to `f32` happens only when building render buffers (relative to an origin, see §8).

### 6.2 Axes

Positions are arrays indexed by the machine's axis list, never hard-coded X/Y/Z.

```rust
pub const MAX_AXES: usize = 9;          // X Y Z A B C U V W — revisit if a machine needs more
pub type Pos = [f64; MAX_AXES];         // unused axes are 0.0 / ignored via the machine's axis mask

pub struct AxisDef {
    pub name: char,                     // 'X', 'Y', 'Z', 'A', 'B', 'C', ...
    pub kind: AxisKind,                 // Linear | Rotary
    pub travel: Option<(f64, f64)>,     // soft limits, machine coords
    pub diameter_mode_capable: bool,    // lathe X
}
```

### 6.3 Channels

A **channel** is an independent stream of program execution (one turret, one spindle group, one program in a mill-turn). The IR is always `Vec<ChannelStream>`; the 3-axis mill simply has exactly one. Synchronization points (wait codes, M-code handshakes) are represented as `Sync` segments so later multi-channel work doesn't require changing the IR shape.

### 6.4 Source references

Every Command, Segment and Diagnostic carries a `SourceRef` so the editor can map clicks and highlights in both directions.

```rust
pub struct SourceRef {
    pub file: u16,        // index into the program's file table (subprograms, includes)
    pub line: u32,        // 1-based
    pub block: u32,       // 0-based block index within the parsed program
    pub col_start: u16,   // optional span for fine-grained highlighting
    pub col_end: u16,
}
```

Canned cycles expand to many primitive Segments that all share the `SourceRef` of the originating block, with a `cycle_step` tag (§6.6).

### 6.5 Commands (dialect → interpreter)

Machine-neutral intents. Dialects emit these; they know nothing about geometry beyond what the program stated. *Provisional.*

```rust
pub enum Command {
    Rapid   { target: AxisWords },
    Linear  { target: AxisWords, feed: Option<Feed> },
    Arc     { target: AxisWords, center: ArcCenter, dir: ArcDir, plane: Plane, feed: Option<Feed> },
    Dwell   { seconds: f64 },
    SetUnits(Units),
    SetDistanceMode(Distance),          // absolute / incremental
    SetPlane(Plane),
    SetWorkOffset(WorkOffsetId),        // G54..G59, G54.1 Pn
    SetFeedMode(FeedMode),              // per-min, per-rev, inverse-time
    SetSpindle { state: SpindleState, rpm: Option<f64>, spindle: u8 },
    ToolChange { tool: u32, offset_h: Option<u32>, offset_d: Option<u32> },
    ToolLengthComp(TlcMode),
    CutterComp  { mode: CompMode, d: Option<u32> },
    Coolant(CoolantState),
    Sync { id: u32 },
    ProgramEnd { rewind: bool },
    Unsupported { what: String },       // always accompanied by a Diagnostic
}
```

`AxisWords` is a sparse set of `(axis_name, value, is_incremental)`. Resolving to absolute positions is the interpreter's job, since it owns the machine position and work offsets.

### 6.6 Segments (the IR)

The central data structure. Consumers (renderer, stats, sim) depend only on this.

```rust
#[repr(u8)]
pub enum SegKind {
    Rapid = 0,
    Linear = 1,
    ArcCw = 2,
    ArcCcw = 3,
    Dwell = 4,
    ToolChange = 5,
    Sync = 6,
}

pub struct Segment {
    pub kind: SegKind,
    pub start: Pos,
    pub end: Pos,
    pub arc: Option<ArcGeom>,           // present for ArcCw / ArcCcw
    pub feed: f64,                      // mm/min; 0.0 for rapid / dwell
    pub spindle_rpm: f64,
    pub tool: u32,
    pub work_offset: WorkOffsetId,
    pub src: SourceRef,
    pub cycle_step: CycleStep,          // None | Approach | FeedIn | Dwell | PeckClear | Retract
    pub flags: SegFlags,                // bitflags: COMP_ACTIVE, OUT_OF_LIMITS, ...
}

pub struct ArcGeom {
    pub center: Pos,                    // absolute; only the two plane axes are meaningful
    pub plane: ArcPlane,                // two indices into the machine's axis list
    pub radius: f64,
    pub turns: u16,                     // full extra revolutions (e.g. P-word); usually 0
}

/// CCW turns from `first` toward `second` (right-hand rule about the third axis).
pub struct ArcPlane { pub first: u8, pub second: u8 }

pub struct WorkOffsetId(pub u16);       // opaque, assigned by the dialect; 0 = machine coords

pub struct ChannelStream {
    pub channel: u8,
    pub segments: Vec<Segment>,
}

pub struct Program {
    pub channels: Vec<ChannelStream>,
    pub diagnostics: Vec<Diagnostic>,
    pub files: Vec<String>,             // file table for SourceRef.file
    pub meta: ProgramMeta,              // units used, tools used, bbox, estimated time
}
```

Design notes:

- **Arcs stay arcs in the IR.** They are exact, compact, and let stats and sim choose their own tolerance. Tessellation is a function in `ir` (`tessellate(&Segment, tol) -> impl Iterator<Item = Pos>`), used by the render-buffer builder and by sim.
- Segments are emitted **in program order**, so a source line maps to a contiguous segment range (binary search on `src.line`). This holds for straight-line, single-file programs. Subprograms, macro loops and `GOTO` (M4) break it, because one line can then produce segments at several places in the stream; the lookup will need a separate line index by then.
- **Arc plane is a pair of axis indices**, not a fixed `XY | ZX | YZ` enum, so the IR names no axes (the machine maps G17/G18/G19 or any other plane selection to indices). The swept angle is the shortest turn in the arc's direction from `start` to `end`, plus `turns` full revolutions; coincident end points mean a full circle. Axes outside the plane move linearly from `start` to `end`, which describes a helix without a separate `helical_delta` field (that field would duplicate `end - start` and could disagree with it).
- A `Segment` is deliberately fat (~250 bytes). If memory becomes an issue on huge programs, the render path already uses a compact encoding (§8) and the IR can later move to a struct-of-arrays layout without changing consumers' API.

### 6.7 Diagnostics

```rust
pub enum Severity { Info, Warning, Error }

pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,        // stable machine-readable id, e.g. UnsupportedGCode, ArcRadiusMismatch
    pub message: String,       // human-readable
    pub src: SourceRef,
    pub dialect_specific: bool,
}
```

Parsing never aborts on the first error. The pipeline recovers at the block boundary and keeps going so the UI can show every problem at once.

## 7. Dialect layer

### 7.1 Data-driven code table

Each dialect ships a TOML file declaring, for every G/M code: its **modal group**, whether it is modal or one-shot, required/optional words, and a handler id for codes that need Rust logic. Example shape:

```toml
[dialect]
id = "fanuc"
variant = "0i"

[[g]]
code = "G01"
group = 1
modal = true
handler = "linear"

[[g]]
code = "G83"
group = 9
modal = true
handler = "cycle_peck_drill"
words = { required = ["Z", "R"], optional = ["Q", "F", "P"] }
```

Adding a Fanuc-compatible variant (Haas, a different Fanuc series) is mostly a table overlay plus a few handler overrides.

### 7.2 Dialect trait (*provisional*)

```rust
pub trait Dialect {
    fn id(&self) -> &'static str;
    fn code_table(&self) -> &CodeTable;

    /// Interpret one parsed block, updating modal state and appending Commands.
    /// Must not panic on bad input; report via `diag`.
    fn interpret_block(
        &self,
        block: &Block,
        state: &mut ModalState,
        out: &mut Vec<Command>,
        diag: &mut Diagnostics,
    );

    /// Hooks for program-level behaviour (subprogram calls, macro variables, end-of-program).
    fn macro_env(&self) -> &dyn MacroEnvironment;
}
```

### 7.3 Canned cycles

Expanded inside the dialect layer into primitive `Command`s (Rapid / Linear / Dwell), each tagged with a `CycleStep`. Downstream code never sees `G81`. The renderer can still colour or filter by `cycle_step`.

### 7.4 Macros and expressions

`gcode-syntax` parses expressions (`#100`, `[#1+2]`, functions) into an AST but does not evaluate them. Evaluation happens in the dialect's `MacroEnvironment`, because variable numbering, system variables and control flow (`IF`, `WHILE`, `GOTO`) differ per controller. Milestone 1 supports only literal numbers and simple variable assignment; the AST is there so adding more doesn't require a parser rewrite.

## 8. Rust → UI boundary

### 8.1 Rule

No JSON for bulk data. The core produces a `RenderBuffer`, a single contiguous little-endian byte block. Tauri returns it as raw bytes (`tauri::ipc::Response`), and the UI wraps it in typed-array views with no copy.

### 8.2 Float precision

GPUs use `f32`. Large coordinates far from origin lose precision. So the buffer carries an `origin: [f64; 3]` and all vertex positions are stored as `f32` **relative to that origin**; the three.js scene applies the offset on the object transform.

### 8.3 Layout (version 1)

All offsets are bytes from the start of the buffer, all integers little-endian, all sections 4-byte aligned.

```
Header (64 bytes)
  0   u32   magic            = 0x47435052 ("GCPR")
  4   u16   version          = 1
  6   u16   flags
  8   u32   vertex_count
  12  u32   segment_count
  16  u32   diag_count
  20  u32   channel_count
  24  f64x3 origin           (24 bytes)
  48  u32   vertices_offset
  52  u32   segments_offset
  56  u32   diags_offset
  60  u32   strings_offset   (UTF-8 pool for diagnostic messages)

Vertices section — struct-of-arrays, ready for GPU upload
  f32 x vertex_count * 3     positions (relative to origin)
  u8  x vertex_count         per-vertex kind (SegKind) for colouring
  (padded to 4 bytes)

Segment table — one entry per Segment (24 bytes)
  u32 first_vertex
  u32 vertex_count           arcs are tessellated into several vertices
  u32 src_line
  u32 src_file_and_block     file:u16 | block-lo:u16 (full block index is recoverable by line lookup)
  u16 tool
  u8  kind                   SegKind
  u8  flags                  SegFlags (low byte)
  f32 feed

Diagnostic table — one entry per Diagnostic (20 bytes)
  u32 src_line
  u16 src_file
  u8  severity
  u8  reserved
  u16 code                   DiagCode
  u16 reserved
  u32 message_offset         into strings pool
  u32 message_length
```

A reverse lookup (editor line → segment range) is a binary search over the segment table, since segments are in program order. The UI does this itself without a round trip.

Layout is versioned from day one. Changing it means bumping `version` and updating the shared test fixture below.

### 8.4 Keeping both sides in sync

The layout is defined **once**, in `core/ir` (Rust), with a generated TypeScript constants file (`ui/src/generated/render_buffer.ts`) produced by a small build step. A golden `.bin` fixture is decoded by both a Rust test and a TypeScript test. If the two ever disagree, CI fails.

### 8.5 Large programs

Interpretation runs off the UI thread (Rust thread on desktop/mobile; Web Worker if the core is ever compiled to WASM). Results stream in chunks (`RenderBuffer` per chunk) for programs of millions of blocks, with progress events. Chunking is a later optimisation but the header's `flags` reserve a bit for "partial / more follows".

## 9. Machine layer (*provisional*)

```rust
pub trait Machine {
    fn name(&self) -> &str;
    fn axes(&self) -> &[AxisDef];
    fn channel_count(&self) -> usize;

    /// Convert machine-axis coordinates into Cartesian tool-tip position + tool axis
    /// (identity for a 3-axis mill; non-trivial for 5-axis or lathes with Y/B axes).
    fn forward_kinematics(&self, pos: &Pos) -> ToolPose;

    fn tool_model(&self, tool: u32) -> Option<&ToolGeometry>;
    fn limits_check(&self, pos: &Pos) -> LimitStatus;

    /// Lathe-style X diameter handling etc. is applied here, not in the dialect.
    fn normalize_axis_word(&self, axis: char, value: f64) -> f64;
}
```

`machine-mill` implements the trivial case. Machine definitions are loaded from data (TOML) where possible: axis list, limits, tool table format, default work offsets.

## 10. Simulation (material removal)

`sim` consumes Segments plus a `Stock` and a `ToolGeometry`, and produces a removable-volume representation. It is organised **per machine family** behind a trait so milling (voxel / dexel) and turning (2D profile revolve) can differ:

```rust
pub trait MaterialSim {
    fn reset(&mut self, stock: &StockDef);
    fn apply(&mut self, seg: &Segment, tool: &ToolGeometry);
    fn mesh(&self, lod: Lod) -> MeshBuffer;     // same binary conventions as §8
}
```

Not started until stages 1–3 are done. Dexels are the leading candidate for milling (good speed/memory, handles 3-axis well); revisit for 5-axis.

## 11. Testing strategy

- **Golden tests** (`tests/golden/`): each `*.nc` has an expected result file. Expected values are small and human-checkable: arc end points, bounding boxes, segment counts, estimated time, diagnostics. Regenerating goldens requires an explicit flag and a reviewed diff.
- **Property tests** (`proptest`): parser never panics on arbitrary bytes; arc tessellation stays within tolerance; incremental/absolute equivalence.
- **Cross-boundary test:** the §8.4 `.bin` fixture decoded in Rust and TS.
- **Dialect conformance:** one test file per modal-group behaviour (e.g. "G91 persists across blocks", "G83 peck depths").
- **Fuzzing** (`cargo-fuzz`) on the lexer/parser from the start. G-code files come from untrusted sources.

## 12. Known risks and how we handle them

| Risk | Mitigation |
|---|---|
| Traits designed from one implementation are wrong in ways only a second one reveals | Add a stub `machine-lathe` and a stub second dialect (a handful of tests) in milestone 2 purely to stress the traits. Budget for revising them. |
| Fanuc behaviour varies by series, options and parameters | Target one documented profile first; surface everything unsupported as a Diagnostic; keep parameters in the dialect TOML. |
| Cutter compensation (G41/G42) is genuinely hard | Defer to after the basic backplot. Emit a `Warning` diagnostic and ignore compensation until implemented. |
| Tauri mobile maturity | Prototype a hello-world with the buffer boundary on Android + iOS *before* building the UI. |
| Heidenhain conversational / non-G-code formats | Parser crate is isolated; a non-G-code front end can produce the same `Command` stream. Not in scope yet. |
| `f32` GPU precision | Origin-relative vertices (§8.2). |

## 13. Milestones

1. **M1: Core skeleton.** Workspace, CI, `gcode-syntax`, `ir`, Fanuc subset (G0/G1/G2/G3, G17–19, G20/21, G90/91, G54–59, F/S/T/M basics), `machine-mill`, interpreter, golden tests. No UI.
2. **M2: Render buffer + minimal UI.** Binary boundary, three.js viewport, CodeMirror editor, line ↔ segment sync, stats and diagnostics panel. Desktop only. Add stub lathe/second dialect to stress the traits.
3. **M3: Mobile spike.** Same app running on Android and iOS; decide on mobile layout.
4. **M4: Machine state.** Tool changes and tool table, G43 length comp, canned cycles G81–G89, subprograms, simple macro variables.
5. **M5: Cutter comp, time estimation with accel model, limits checking.**
6. **M6: Material removal (mill).**
7. **M7+: Additional dialects (Haas, then others) and machine types (lathe, mill-turn).**

## 14. Disclaimer

This software visualizes and analyzes G-code. It is not a safety verification tool. Always verify programs independently before running them on a machine.
