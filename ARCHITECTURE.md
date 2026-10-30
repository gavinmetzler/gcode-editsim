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

The interpreter's entry point is `interpret(source, file_name, &dyn Dialect, &dyn Machine, Options) -> Program`. It never fails: every problem is a diagnostic. Behaviour decided in M1:

- **Positions are machine coordinates.** Work offsets come from the machine definition and are applied by the interpreter; incremental moves are relative to the current machine position. An undefined work offset is reported once and machine coordinates are used.
- **The machine starts at its reference (home) position** from the machine definition (machine zero on axes that give none).
- **A move is only drawn from a known position.** When the dialect reports that the machine went somewhere it cannot model (`Command::PositionUnknown`: an unsupported canned cycle, subprogram or coordinate mode, an unknown G code, a return to a reference position the machine file does not describe), those axes become unknown. Later moves are left out, with one warning, until every axis has been given an absolute position again. Nothing is drawn from a stale starting point.
- **Reference returns and machine-coordinate moves** (`ReturnToReference`, `MachineRapid`) are carried out against the machine definition's reference positions and machine coordinates.
- **Full circles are decided exactly.** An offset-style arc whose end is within 1e-6 mm of its start (floating-point drift from incremental moves is ~1e-10 mm; the smallest programmable step is 0.001 mm) has its end set exactly to the start, so the IR's coincident-end-points rule applies. Without this, drift could turn a full circle into a near-zero arc.
- **Arc centres** are computed here. Offset-style centres are checked so that start and end lie within 0.01 mm of the same radius (real controls make this a parameter); R-style centres are solved for the short or long arc by the sign of R. An impossible arc is an error and the block does not move.
- **Travel limits** are checked for every motion segment, including arc bulges (via the segment's bounding box). Offending segments get `OUT_OF_LIMITS` and a warning; after 50 warnings the rest are only flagged.
- **Program end** stops execution: later blocks with content are not run, with one info note.
- **Block delete** (`/`) is an option (`Options::block_delete`), off by default, matching the operator switch.
- **Events** (program and optional stops, spindle start/stop with direction, coolant on/off, program end) become zero-length segments at the current position, in program order, one per command the program gives (a repeated command is recorded again). An event that happens while the position is unknown is flagged `POSITION_UNKNOWN`, because its position is only the last known one.

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

**Dependency rules (enforced in CI by `tools/depcheck`).** Each crate may use only the workspace crates listed for it below, as a normal or build dependency (platform-specific ones included):

| Crate | May depend on |
|---|---|
| `gcode-syntax` | nothing in the workspace |
| `ir` | nothing in the workspace |
| `dialect-api` | `gcode-syntax`, `ir` |
| `dialect-fanuc` (and future dialects) | `dialect-api`, `gcode-syntax`, `ir` |
| `machine-api` | `ir` |
| `machine-mill` (and future machines) | `machine-api`, `ir` |
| `interpreter` | `dialect-api`, `machine-api`, `gcode-syntax`, `ir` |
| `sim` | `machine-api`, `ir` |
| `testkit` | `ir` |

The principles behind the table:

- The dialect side and the machine side never depend on each other.
- `interpreter` and `sim` use only the `*-api` crates, never a concrete dialect or machine, so those stay swappable.
- Test-only (dev) dependencies may use any `core/` crate, because tests wire concrete dialects and machines together, but never anything outside `core/`.
- `app` and `ui` may depend on `core`; `core` never depends on them.

The table lives in `tools/depcheck/rules.toml`. The check reads `cargo metadata`, so renamed, platform-specific and workspace-inherited dependencies are covered. A new crate must be added to the table before CI passes. Changing the table is an architecture change: update this section in the same PR.

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

pub struct AxisDef {                    // in machine-api
    pub name: char,                     // 'X', 'Y', 'Z', 'A', 'B', 'C', ...
    pub kind: AxisKind,                 // Linear | Rotary
    pub travel: Option<(f64, f64)>,     // soft limits, machine coords
    pub arc_offset_letter: Option<char>,// e.g. 'I' for X; None = not in any arc plane
    pub direction: Option<CartesianAxis>, // x | y | z: travel direction (linear) or rotation axis (rotary)
    pub rapid_rate: Option<f64>,        // mm/min or deg/min
    pub reference: Option<f64>,         // reference (home) position, machine coords
    pub diameter_mode_capable: bool,    // turning X
}
```

The arc-offset letter belongs to the machine definition, not the dialect, so a dialect never has to assume that I goes with X.

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

Machine-neutral intents, defined in `dialect-api`. Dialects emit these; they know nothing about geometry beyond what the program stated. All values are already in internal units. *Provisional.*

```rust
pub enum Command {
    Rapid   { target: Vec<AxisWord> },
    Linear  { target: Vec<AxisWord> },
    Arc     { target: Vec<AxisWord>, center: ArcCenter, dir: ArcDir, plane: ArcPlane },
    Dwell   { seconds: f64 },
    SetFeedRate(f64),                   // emitted whenever F is programmed
    SetUnits(LengthUnit),               // records the program's units; values are already mm
    SetDistanceMode(DistanceMode),      // absolute / incremental
    SetPlane(ArcPlane),
    SetWorkOffset(WorkOffsetId),
    SetFeedMode(FeedMode),              // per-min, per-rev, inverse-time
    SetSpindleSpeed { spindle: u8, rpm: f64 },
    SetSpindleState { spindle: u8, state: SpindleState },   // off / cw / ccw
    SelectTool(u32),                    // choose the next tool
    ToolChange { tool: u32 },           // change tools now
    ReturnToReference { via: Vec<AxisWord>, position: u8 }, // rapid via a point to reference position n
    MachineRapid { target: Vec<AxisWord> }, // rapid in machine coordinates
    PositionUnknown { axes: Vec<usize>, reason: String },   // the machine moved somewhere not simulated
    Coolant(CoolantState),
    Stop { optional: bool },            // operator stop / optional stop
    ProgramEnd { rewind: bool },
    Unsupported { what: String },       // always accompanied by a Diagnostic
}

pub struct AxisWord { pub axis: usize, pub value: f64, pub incremental: bool }  // axis = index into AxisSet
pub enum ArcCenter { Offset(Vec<AxisOffset>), Radius(f64) }  // offsets from start; negative R = long arc
```

Resolving axis words to absolute positions is the interpreter's job, since it owns the machine position and work offsets. Commands carry no `SourceRef`: the caller attaches the source block's reference to every command the block produced.

Differences from the first draft, and why:

- **Feed is its own command** rather than an optional field on each move. `F` can appear without motion, and a single source of truth avoids two ways to say the same thing.
- **Spindle speed and spindle state are separate**, because `S` and the start/stop codes are independent and often appear on different blocks.
- **Tool selection and tool change are separate** (`T` chooses, a change code changes), which most controllers distinguish. A dialect whose `T` word changes immediately just emits both.
- **`Stop` was added** for operator stops and optional stops.
- **`ReturnToReference`, `MachineRapid` and `PositionUnknown` were added** (M1 hardening) so that reference returns and machine-coordinate moves are simulated, and so that a move the dialect cannot simulate never leaves the interpreter drawing later moves from the wrong place.
- **Deferred until their milestones**: tool length and cutter compensation (M4/M5) and multi-channel `Sync`.

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
    Stop = 7,                           // program stop
    OptionalStop = 8,
    SpindleCw = 9,
    SpindleCcw = 10,
    SpindleStop = 11,
    CoolantFlood = 12,
    CoolantMist = 13,
    CoolantOff = 14,
    ProgramEnd = 15,
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
    pub flags: SegFlags,                // bitflags: COMP_ACTIVE, OUT_OF_LIMITS, POSITION_UNKNOWN, ...
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

- **Events are segments.** Everything that is not motion (dwell, tool change, stops, spindle, coolant, program end, `Sync`) is a zero-length segment with `start == end`, rather than a separate event list. That keeps one list in program order, so the line lookup below finds a line's events as well as its moves, and a later playback can walk a single list. Each event is its own kind, because the render buffer stores only the kind byte. `SegKind` and `SegFlags` values are fixed once the render buffer exists.
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
    fn id(&self) -> &str;
    fn code_table(&self) -> &CodeTable;

    /// The controller's power-on modal state.
    fn initial_state(&self, axes: &AxisSet) -> ModalState;

    /// Interpret one parsed block, updating modal state and appending Commands.
    /// Must not panic on bad input; report via `diag`.
    fn interpret_block(
        &self,
        block: &Block,
        ctx: &BlockContext,             // the machine's AxisSet + the block's SourceRef
        state: &mut ModalState,
        out: &mut Vec<Command>,
        diag: &mut Diagnostics,
    );
}
```

`AxisSet` is the machine's ordered axis list as the dialect sees it: each axis's address letter, whether it is rotary, and optionally the letter for its arc-centre offset (e.g. `I` for `X`). Index *i* in the set is index *i* in `Pos`. The dialect looks letters up there instead of assuming X/Y/Z exist; it cannot depend on `machine-api` (which comes later in the dependency order), so the caller builds the `AxisSet` from the user's machine definition (§9).

A `MacroEnvironment` hook (subprograms, macro variables, control flow) is deferred until macros are in scope (M4); see §7.4.

### 7.3 Canned cycles

Expanded inside the dialect layer into primitive `Command`s (Rapid / Linear / Dwell), each tagged with a `CycleStep`. Downstream code never sees `G81`. The renderer can still colour or filter by `cycle_step`.

### 7.3a Unsupported modes that affect later blocks

Some unsupported codes change how *later* blocks move: coordinate shifts (G92, G52, G10), rotation, scaling, mirroring, polar input, subprograms, and unknown G codes. Reporting only the code's own block would leave every following move drawn in the wrong place without a diagnostic. Such codes start a **motion blocker** (`ModalState::motion_blockers`): an error explains, once, that moves are not shown from that line until the matching cancel code (or the end of the program). While a blocker is in effect the dialect emits no motion, only `PositionUnknown`, so when the blocker ends the interpreter waits for a known position before drawing again.

### 7.4 Macros and expressions

`gcode-syntax` parses expressions (`#100`, `[#1+2]`, functions) into an AST but does not evaluate them. Evaluation happens in the dialect's `MacroEnvironment`, because variable numbering, system variables and control flow (`IF`, `WHILE`, `GOTO`) differ per controller. Milestone 1 evaluates literal numbers only: a block that uses a variable or expression gets an error diagnostic and is skipped. The AST is there so adding evaluation later doesn't require a parser rewrite.

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
    fn channel_count(&self) -> usize { 1 }

    /// Convert machine-axis coordinates into Cartesian tool-tip position + tool axis
    /// (identity for a 3-axis mill; non-trivial for 5-axis or lathes with Y/B axes).
    fn forward_kinematics(&self, pos: &Pos) -> ToolPose;

    /// Axes of `pos` outside their travel limits (empty if none).
    fn check_limits(&self, pos: &Pos) -> Vec<LimitViolation>;

    /// Machine-coordinate origin of a work coordinate system (id 0 = machine coords).
    fn work_offset(&self, id: WorkOffsetId) -> Option<Pos>;

    fn speeds(&self) -> &SpeedLimits;   // max feed, max spindle rpm

    /// Lathe-style X diameter handling etc. is applied here, not in the dialect.
    fn normalize_axis_value(&self, axis: usize, value: f64) -> f64 { value }
}
```

`machine-mill` implements the trivial case. `tool_model` (tool geometry and the tool table) is deferred to M4, when tool changes and length compensation are simulated.

Machine definitions are **user-editable text files** (TOML) describing the user's own machine: name, machine family, axis list (letters, kinds, directions, arc-offset letters, travel limits, rapid rates), maximum feed and spindle speed, and work offsets. `machine-api` loads and validates the file into a `MachineDef` (unknown settings are rejected, not ignored); the machine type named by `family` then checks it makes sense for that type (e.g. `Mill::new`). The format is documented for users in `docs/machine-definition.md`. A UI for editing them comes later; until then users edit the file directly. The dialect's `AxisSet` (§7.2) is built from this definition by the interpreter.

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

- **Golden tests** (`tests/golden/`): each `*.nc` has an expected result file per stage: `<name>.syntax.txt` (parse result) and `<name>.ir.json` (interpreted with the Fanuc dialect on the generic 3-axis mill). The IR file is a compact summary built by `testkit` (end points by axis letter, arc centres and radii, feed, tool, flags, diagnostics, bounding box), rounded to 1e-6, rather than the full `Segment` structs, so diffs stay small and human-checkable. Regenerating goldens requires an explicit flag (`UPDATE_GOLDEN=1`) and a reviewed diff.
- **Property tests** (`proptest`): parser never panics on arbitrary bytes; arc tessellation stays within tolerance; incremental/absolute equivalence.
- **Cross-boundary test:** the §8.4 `.bin` fixture decoded in Rust and TS.
- **Dialect conformance:** one test file per modal-group behaviour (e.g. "G91 persists across blocks", "G83 peck depths").
- **Fuzzing** (`cargo-fuzz`) on the lexer/parser from the start. G-code files come from untrusted sources. CI runs the parser target for 60 seconds on every push and PR. It is seeded with the golden programs and uses a pinned nightly and cargo-fuzz version. Any panic, stack overflow, input taking over 10 s, or memory use over 2 GB fails the job, and the crashing input is uploaded. Longer runs are done locally.

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
