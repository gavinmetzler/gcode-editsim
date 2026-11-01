# ARCHITECTURE

Status: **living document**, kept in step with the code. M1 (core skeleton), M2 (render buffer and minimal desktop UI) and M3 (a dependable backplotter) are complete; M4 is next (§13). Where this file describes something not built yet, it says which milestone brings it. Interfaces marked *provisional* were checked against a stub second dialect (Osai 10 Series) and a stub second machine type (lathe) in M2.2 (findings in §7.2 and §9); they may still change when those become real (M8).

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
  → dialect        : Blocks → Commands (machine-neutral intents), tracks modal state,
                     emits Diagnostics; will expand canned cycles (M3.6, §7.3)
  → interpreter    : applies Commands to a Machine model → Segments (IR), per channel
  → consumers      : render buffer → backplot and statistics (built); time estimate (built, M3.8; with acceleration, M6),
                     material removal (M7)
```

The interpreter's entry point is `interpret(source, file_name, &dyn Dialect, &dyn Machine, Options) -> Program`. It never fails: every problem is a diagnostic. Behaviour decided in M1:

- **Positions are machine coordinates.** Work offsets come from the machine definition and are applied by the interpreter; incremental moves are relative to the current machine position. An undefined work offset is reported once and machine coordinates are used.
- **The machine starts at its reference (home) position** from the machine definition (machine zero on axes that give none).
- **A move is only drawn from a known position.** When the dialect reports that the machine went somewhere it cannot model (`Command::PositionUnknown`: an unsupported canned cycle, subprogram or coordinate mode, an unknown G code, a return to a reference position the machine file does not describe), those axes become unknown. Later moves are left out, with one warning, until every axis has been given an absolute position again. Nothing is drawn from a stale starting point.
- **Reference returns and machine-coordinate moves** (`ReturnToReference`, `MachineRapid`) are carried out against the machine definition's reference positions and machine coordinates.
- **Full circles are decided exactly.** An offset-style arc whose end is within 1e-6 mm of its start (floating-point drift from incremental moves is ~1e-10 mm; the smallest programmable step is 0.001 mm) has its end set exactly to the start, so the IR's coincident-end-points rule applies. Without this, drift could turn a full circle into a near-zero arc.
- **Arc centres** are computed here. Offset-style centres are checked so that start and end lie within 0.01 mm of the same radius (real controls make this a parameter); R-style centres are solved for the short or long arc by the sign of R. An impossible arc is an error and the block does not move.
- **Travel limits** are checked for every motion segment, including arc bulges (via the segment's bounding box). Offending segments get `OUT_OF_LIMITS` and a warning; after 50 warnings the rest are only flagged.
- **Program end** stops execution: later blocks with content are not run, with one info note, unless they are a subprogram the program calls (see §7.2, "Subprograms").
- **Block delete** (`/`) is an option (`Options::block_delete`), off by default, matching the operator switch.
- **Positions in program coordinates (M4.5).** `interpret_traced(…, lines)` returns the same `Program` plus a `BlockTrace` for each run of each block on the given lines, in run order: the modal state after the block (so a G91, G20 or G55 in the block is the mode its words were read in), the commands the dialect made, whether it reported an error, and the full position before and after in the block's **work coordinates** (machine position less the origin of the work offset in effect for the block, for both), in mm and degrees, with the program's units in the state. Whether every axis's position was known before and after is recorded too. A subprogram block has one entry per call. A plain `interpret` records nothing (one check per block); positions on a lathe's diameter axis are radius values, as the machine model holds them (M8).
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
│  ├─ dialect-osai/            # Osai 10 Series stub (M2), to exercise the dialect traits
│  ├─ machine-api/             # Machine trait, AxisDef, machine-file loader (tool and stock models come with M6/M7)
│  ├─ machine-mill/            # first real machine (3-axis)
│  ├─ machine-lathe/           # 2-axis lathe stub (M2), to exercise the machine traits
│  ├─ interpreter/             # glue: dialect + machine + state → IR
│  ├─ transform/               # rewrites selected lines exactly: text edits by byte span (M4)
│  ├─ sim/                     # material removal, per machine family (placeholder until M7)
│  └─ testkit/                 # golden-file helpers (compact summary, compare, UPDATE_GOLDEN)
├─ app/
│  ├─ backend/                 # the app's behaviour without Tauri: machine/dialect choice, interpret → render buffer, file access
│  └─ shell/                   # Tauri 2 shell: thin commands over backend (its own Cargo workspace, so the rest builds without WebKit)
├─ ui/                         # TypeScript UI: three.js viewport, CodeMirror editor, panels (§8.6)
├─ tools/
│  ├─ depcheck/                # the dependency-rule check below
│  ├─ gen-ts/                  # generates the UI's render-buffer constants (§8.4)
│  ├─ corpus/                  # runs real (private) programs and groups their diagnostics (M3.0)
│  ├─ bench/                   # times the pipeline on a large program (M3.9)
│  └─ ci/                      # which CI jobs a pull request needs
├─ packaging/                  # Flatpak package and the app icon (§8.7)
├─ docs/                       # machine file format, dialect notes, milestone plans
└─ tests/
   └─ golden/                  # *.nc input + expected .syntax.txt / .ir.json output, render-buffer .bin fixture
```

**Dependency rules (enforced in CI by `tools/depcheck`).** Each crate may use only the workspace crates listed for it below, as a normal or build dependency (platform-specific ones included):

| Crate | May depend on |
|---|---|
| `gcode-syntax` | nothing in the workspace |
| `ir` | nothing in the workspace |
| `dialect-api` | `gcode-syntax`, `ir` |
| `dialect-fanuc`, `dialect-osai` (and future dialects) | `dialect-api`, `gcode-syntax`, `ir` |
| `machine-api` | `ir` |
| `machine-mill`, `machine-lathe` (and future machines) | `machine-api`, `ir` |
| `interpreter` | `dialect-api`, `machine-api`, `gcode-syntax`, `ir` |
| `transform` | `dialect-api`, `machine-api`, `gcode-syntax`, `interpreter`, `ir` |
| `sim` | `machine-api`, `ir` |
| `testkit` | `ir` |
| `gen-ts` (`tools/gen-ts`) | `ir` |
| `corpus` (`tools/corpus`), `bench` (`tools/bench`) | `app-backend`, `interpreter`, `ir` |
| `depcheck` (`tools/depcheck`) | nothing in the workspace |
| `app-backend` (`app/backend`) | `interpreter`, `ir`, the `*-api` crates, and the concrete dialects and machines |

The principles behind the table:

- The dialect side and the machine side never depend on each other.
- `interpreter`, `transform` and `sim` use only the `*-api` crates, never a concrete dialect or machine, so those stay swappable.
- Test-only (dev) dependencies may use any `core/` crate, because tests wire concrete dialects and machines together, but never anything outside `core/`.
- `app` and `ui` may depend on `core`; `core` never depends on them. `app-backend` is the one place that wires concrete dialects and machines together for the app: it picks the machine type from the machine file's `family` and the dialect from its id. It may use the file system; `core` may not. The Tauri shell (`app/shell`) is a separate Cargo workspace that depends only on `app-backend` and Tauri, so it is not in this table.

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
    CallSubprogram { program: u32, repeat: u32 }, // run a program in the same file (M3.7)
    ReturnFromSubprogram,               // M99: back to after the call, or run again
    Unsupported { what: String },       // always accompanied by a Diagnostic
}

pub struct AxisWord { pub axis: usize, pub value: f64, pub incremental: bool }  // axis = index into AxisSet
pub enum ArcCenter {
    Offset(Vec<AxisOffset>),   // offsets from the start (Fanuc I/J/K)
    Radius(f64),               // negative R = long arc
    Position(Vec<AxisWord>),   // a point, like an end point: absolute in work coordinates or incremental from the start (Osai I/J)
}
```

Resolving axis words to absolute positions is the interpreter's job, since it owns the machine position and work offsets. Commands carry no `SourceRef`: the caller attaches the source block's reference to every command the block produced.

Differences from the first draft, and why:

- **Feed is its own command** rather than an optional field on each move. `F` can appear without motion, and a single source of truth avoids two ways to say the same thing.
- **Spindle speed and spindle state are separate**, because `S` and the start/stop codes are independent and often appear on different blocks.
- **Tool selection and tool change are separate** (`T` chooses, a change code changes), which most controllers distinguish. A dialect whose `T` word changes immediately just emits both.
- **`Stop` was added** for operator stops and optional stops.
- **`ReturnToReference`, `MachineRapid` and `PositionUnknown` were added** (M1 hardening) so that reference returns and machine-coordinate moves are simulated, and so that a move the dialect cannot simulate never leaves the interpreter drawing later moves from the wrong place.
- **Deferred until their milestones**: cutter compensation (M6) and multi-channel `Sync`. Tool length (height) and wear compensation are not planned (§13).
- **`ArcCenter::Position` was added** (M2.2) because the Osai 10 Series programs the centre as a position in G90. The dialect cannot turn that into an offset, since only the interpreter knows the start point and the work offset. The interpreter converts its values like axis words (work offset, `normalize_axis_value`), so a centre on a diameter axis is a diameter, as on that control. `Offset` values are not converted by the machine (Fanuc gives I as a radius on lathes).

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
    pub time: Option<f64>,              // estimated seconds, no acceleration (M3.8); None if unknown
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
- **Estimated time per segment (M3.8).** The interpreter sets `time`: a feed move's length over the linear axes (`ir::path_length`, along the arc for arcs) divided by its feed, capped at the machine's `max_feed`; under feed per revolution the feed is F × spindle rpm; a rapid takes as long as its slowest axis at that axis's rapid rate; a dwell its seconds; a tool change the machine's `tool_change_time`; other events none. No acceleration (that is M6). `None` when it cannot be worked out (no rapid rate for a moving axis, no feed rate, only rotary axes feeding, a dwell of unknown length). `ProgramMeta::estimated_time` is the sum of the known times. The UI adds them up per tool for the statistics dialog.
- **Arcs stay arcs in the IR.** They are exact, compact, and let stats and sim choose their own tolerance. Tessellation is a function in `ir` (`tessellate(&Segment, tol) -> impl Iterator<Item = Pos>`), used by the render-buffer builder and by sim. It gives the exact start and end, points on the arc with chord error ≤ `tol`, at least one point every 90°, and at most `MAX_ARC_POINTS` (100 000) per arc; an unusable tolerance gets the maximum.
- Segments are emitted **in the order the program runs**. For a straight-line program that is line order, so a line's segments are one contiguous range (`ir::segments_for_line`, binary search). Subprograms (M3.7) break this: a subprogram line produces segments at every call. The UI therefore builds a **line index** (`buildLineIndex` in `ui/src/editor/lines.ts`: segment indices sorted by line, then run order), which gives every run of a line, and where the tool is after it (the end of its first run).
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

    /// The program number if `block` starts a program (Fanuc `O1000` or
    /// `:1000`), so the interpreter can follow calls within the file. The
    /// default is `None`: a dialect without subprograms (Osai stub).
    fn program_number(&self, block: &Block) -> Option<u32> { None }

    /// Reading and writing numbers as the controller does (M4.4), for
    /// rewriting lines. Both default to `None`: no rewriting.
    fn read_value(&self, number: &Number, kind: ValueKind, units: LengthUnit) -> Option<f64> { None }
    fn format_value(&self, value: f64, kind: ValueKind, units: LengthUnit, style: &NumberStyle) -> Option<String> { None }

    /// Why a transformation cannot rewrite a block (M4.6): codes whose effect
    /// its axis words do not carry (Fanuc: canned cycles, G92/G52/G10 and the
    /// other ignored shifts, G53, G28/G30). Default `None`.
    fn transform_refusal(&self, block: &Block, state: &ModalState) -> Option<String> { None }
}
```

`AxisSet` is the machine's ordered axis list as the dialect sees it: each axis's address letter, whether it is rotary, and optionally the letter for its arc-centre offset (e.g. `I` for `X`). Index *i* in the set is index *i* in `Pos`. The dialect looks letters up there instead of assuming X/Y/Z exist; it cannot depend on `machine-api` (which comes later in the dependency order), so the caller builds the `AxisSet` from the user's machine definition (§9).

**Subprograms (M3.7)** are control flow, so the interpreter follows them, not the dialect: the dialect numbers the programs in the file (`program_number`) and turns M98/M99 into `Command::CallSubprogram { program, repeat }` and `ReturnFromSubprogram`; the interpreter runs blocks with a program counter and a call stack (at most `MAX_CALL_DEPTH` = 10 deep, and `Options::max_blocks` blocks in all, 2,000,000 by default). A call to a program not in the file gives a warning and is skipped; running on into another program's number block ends the main program (or returns from a subprogram); M99 in the main program ends it, drawn once; blocks never run (after the end, and not in a called subprogram) get one note. Diagnostics repeated by a subprogram's runs are reported once. A `MacroEnvironment` hook (macro variables, `IF`/`WHILE`/`GOTO`) stays deferred (`docs/WISHLIST.md`); see §7.4.

**What the second dialect showed (M2.2).** `dialect-osai`, a stub of the Osai 10 Series, implements the same `Dialect` trait from its own code table, with no Fanuc code. G17-G19 there pick planes by position in the axis list rather than by letter, which `AxisSet` supports as it is. One interface change was needed: `ArcCenter::Position` (§6.5). Other findings, recorded rather than fixed:

- **The parser's lexical rules are Fanuc-shaped.** On the 10 Series, parentheses hold commands (`(UAO,1)`), not comments. The dialect reads them from the parser's parenthesis comments, but a `)` inside a quoted message breaks the line. Labels (`"START"`), axis names without a value (`G X Y`), `>>` and `E10=5` are syntax errors. Letters are folded to upper case, so Osai's lower-case `r` (corner radius) and `t` (block time) arrive as `R` and `T`. A real Osai dialect (M8) needs a per-dialect lexical profile in `gcode-syntax` (comment characters, case, labels, assignments). Until then the Osai stub treats an unreadable line as "position unknown", because the control may well have moved there.
- **`arc_offset_letter` is a Fanuc-family idea.** Osai names the centre I and J by plane position whatever the axes, so it ignores the machine's arc-offset letters. They stay in the machine file for dialects that name centres per axis.
- **`Command::Arc` cannot say how many turns.** Osai's helix pitch K can mean several turns; `Arc` has no turns field (the IR's `ArcGeom::turns` exists). The stub refuses K.
- **Tolerances belong to the control.** Osai's full-circle threshold (FCT, 0.001 mm by default) differs from the interpreter's fixed 1e-6 mm; its radius tolerance (CET, 0.01 mm) matches. Both may need to come from the dialect or the machine.
- **Coolant kinds.** `CoolantState` has flood and mist; Osai's M7 "auxiliary coolant" is shown as mist.
- **Shared block skeleton.** Both dialects collect words, check groups and report unused words in nearly the same way. That code could move to `dialect-api` helpers, as a separate refactoring once the second real dialect exists.

**Rewriting numbers (M4.4).** A transformation changes values, so the text must be written back as the controller will read it. The parser keeps each word's span, and `gcode_syntax::NumberStyle` reads how its number was written (sign, a gap after the sign, leading zeros, decimal point, decimals); writing a value in that style is lexical and lives there too. What a number *means* is the dialect's: `read_value` turns a literal into mm or degrees, and `format_value` writes a value back so that reading it gives the value within half of the least input increment. For Fanuc both go through the same functions the interpreter uses (`dialect-fanuc/src/numbers.rs`): a literal with a decimal point keeps one, with at least the decimals it had and more as needed up to the increment (3 in mm and degrees, 4 in inches); one without keeps none, counted in least increments (DPI = 0) or whole units (calculator input, which needs a point after all when the value is not whole). The `transform` crate applies the result as text edits by byte span (`apply_edits`), touching nothing else, and `rewrite_word` makes no edit at all when the new text reads back as the value already there, so rewriting a word with its own value never changes it. A dialect that does not implement the two methods (the Osai stub) cannot be rewritten.

**Translate (M4.6).** `transform::transform(source, dialect, machine, options, lines, &Transformation::Translate { offset })` gives the edits and notes, or refuses with the first line that cannot be done (nothing is changed then). It runs the program with a trace of the selection (widened until it reaches the move after the selection, and to the end of the file when the selection calls a subprogram) and walks the trace in run order, following the position the *rewritten* program will reach: a selected move must end at its original end plus the offset (G90 words get the new value; G91 words the new value less where the rewritten program is; an axis not written gets a word only if it now has to change, added after the block's last axis word in its style); blocks run in between that are not selected are replayed from the axis words they give; the first move after the selection keeps its original end if it is incremental (adjusted the same way, with a note naming it) and is left alone if absolute (a note says its path changes). Written values are read back through the dialect, so rounding never accumulates. I/J/K and R are unchanged; an arc cannot be the first move of the selection or the move after it (one end would move and not the other), which is refused with a hint. Refused as well: errors, unknown positions, G53, G28/G30, the dialect's `transform_refusal`, values that are not plain numbers, an axis written twice, offsets on rotary axes or on axes the machine lacks. A subprogram line that runs more than once, an M98 in the selection, and selected lines that never run get notes. Rotate (M4.7) and Scale (M4.8) are further cases of `Transformation`.

### 7.3 Canned cycles

*On hold (owner decision, 2026-10-31; left to the community). Today each hole of a canned cycle is drawn as a rapid to its position in the working plane, with one warning per cycle code (§7.3a).* A full implementation would expand the cycle inside the dialect layer into primitive `Command`s (Rapid / Linear / Dwell), each tagged with a `CycleStep`. Downstream code never sees `G81`. The renderer can still colour or filter by `cycle_step`.

### 7.3a Codes that change later moves: never hide moves

Some unsupported codes change how *later* blocks move: coordinate shifts (G92, G52, G10, Osai presetting and origins), rotation, scaling, mirroring, polar input, macro and subprogram calls, and unknown G codes. **Moves are never hidden** (owner decision, 2026-10-31): a backplot drawn without their effect is more useful than none, and the diagnostics say not to rely on it. Each such code gives one diagnostic where it appears (a warning; an error for an unknown G code), its words are data rather than a move, and later moves are drawn as programmed. Canned cycles draw their hole positions (§7.3).

Until M3.0–M3.5 these codes started a *motion blocker* that hid every later move until a cancel code; that mechanism is gone. A position that is genuinely unknown (G30 to a reference the machine file does not give, an Osai line that cannot be read) still makes the interpreter wait for an absolute position on every axis before drawing again: nothing is drawn from a guessed starting point.

Coordinate shifts and canned cycles are on hold, left to the community (`docs/WISHLIST.md`).

### 7.4 Macros and expressions

`gcode-syntax` parses expressions (`#100`, `[#1+2]`, functions) into an AST but does not evaluate them. Evaluation happens in the dialect's `MacroEnvironment`, because variable numbering, system variables and control flow (`IF`, `WHILE`, `GOTO`) differ per controller. So far only literal numbers are evaluated: a block that uses a variable or expression gets an error diagnostic and is skipped. Simple variables are planned with subprograms (M3.7). The AST is there so adding evaluation later doesn't require a parser rewrite.

## 8. Rust → UI boundary

### 8.1 Rule

No JSON for bulk data. The core produces a `RenderBuffer`, a single contiguous little-endian byte block. Tauri returns it as raw bytes (`tauri::ipc::Response`), and the UI wraps it in typed-array views with no copy.

### 8.2 Float precision

GPUs use `f32`. Large coordinates far from origin lose precision. So the buffer carries an `origin: [f64; 3]` and all vertex positions are stored as `f32` **relative to that origin**; the three.js scene applies the offset on the object transform. The origin is the **centre of the bounding box of all vertices** (non-finite ones ignored; zero if there are none), which keeps the largest relative value, and so the `f32` error, as small as possible.

### 8.3 Layout (version 2)

All offsets are bytes from the start of the buffer, all integers little-endian, all sections 4-byte aligned.

```
Header (64 bytes)
  0   u32   magic            = 0x47435052 ("GCPR" as a number; the bytes are 52 50 43 47, "RPCG")
  4   u16   version          = 2
  6   u16   flags            bit 0: partial, more chunks follow (reserved, §8.5)
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

Segment table — one entry per Segment (28 bytes)
  u32 first_vertex
  u32 vertex_count           arcs are tessellated into several vertices
  u32 src_line
  u32 src_file_and_block     file in the high 16 bits, block index low 16 bits in the low 16 (full block index is recoverable by line lookup)
  u16 tool                   tools above 65535 are stored as 65535
  u8  kind                   SegKind
  u8  flags                  SegFlags (low byte)
  f32 feed                   mm/min (feed per revolution already multiplied by the spindle speed)
  f32 time                   estimated seconds (Segment::time); NaN if unknown. Added in version 2 (M3.8)

Diagnostic table — one entry per Diagnostic (20 bytes)
  u32 src_line
  u16 src_file
  u8  severity               0 info, 1 warning, 2 error
  u8  reserved
  u16 code                   DiagCode
  u16 reserved
  u32 message_offset         into strings pool
  u32 message_length
```

**Vertices per segment.** A straight move has 2 vertices (start, end), an arc has its tessellated points (chord error ≤ the caller's tolerance), and an event (dwell, tool change, stop, spindle, coolant, program end) has 1, its position. Each segment's vertices are its own: the end of one move and the start of the next are stored twice, so a segment is drawn and picked on its own. The caller supplies the arc tolerance and the mapping from machine axes to x/y/z (the machine's forward kinematics), so `ir` stays free of axis names.

**Channels.** Segments of all channels are written in channel order. Version 2 has no per-segment channel number, so a multi-channel program needs a new version (planned with multi-channel work).

The layout is implemented in `ir::render` (`encode_render_buffer`; offsets in `ir::render::layout`). The shared fixture is `tests/golden/render_buffer_v2.bin`, built from a small hand-made program in `core/ir/tests/render_buffer.rs`, whose expected values are worked out by hand there.

A reverse lookup (editor line → segment range) is a binary search over the segment table, since segments are in program order. The UI does this itself without a round trip.

Layout is versioned from day one. Changing it means bumping `version` and updating the shared test fixture below.

### 8.4 Keeping both sides in sync

The layout is defined **once**, in `core/ir` (Rust), with a generated TypeScript constants file (`ui/src/generated/render_buffer.ts`) produced by a small build step. A golden `.bin` fixture is decoded by both a Rust test and a TypeScript test. If the two ever disagree, CI fails.

As built (M2.4):

- `tools/gen-ts` writes the constants from `ir::render::layout` and the value lists `SegKind::ALL`, `SegFlags::NAMED`, `Severity::ALL` and `DiagCode::ALL` (each guarded by a test that fails when a value is missing). Run `cargo run -p gen-ts` after a layout change. Its test fails when the committed file differs from what it would generate, so `cargo test --workspace` catches a forgotten regeneration.
- `ui/src/buffer/decode.ts` checks every count and offset before reading (a damaged buffer is a `DecodeError`, never an out-of-range read), then exposes positions and vertex kinds as zero-copy typed-array views over the bytes. On a big-endian host, which no supported platform is, it copies the positions instead.
- `ui/tests/decode.test.ts` checks the fixture's values, the same hand-worked ones as `core/ir/tests/render_buffer.rs`.

### 8.4a The desktop app's commands (M2.3)

The Tauri shell (`app/shell`) forwards to `app-backend` and holds no G-code logic. Bulk data is raw bytes in **both** directions:

- `interpret`: the program is the raw request body; the reply is the render buffer as raw bytes (`tauri::ipc::Response`). The machine, dialect and options are chosen beforehand (`select_machine`, `set_run_options`) and kept in the app's state, so a run carries only the program.
- `load_file` replies with the file's raw bytes; `save_file` takes them as the raw body, with the path in an `x-path` header (percent-encoded UTF-8, as `encodeURIComponent` writes it, since header values cannot carry arbitrary text).
- `list_dialects`, `list_machines`, `machine_folder` and the two setters are small JSON.
- `remembered_settings` and `remember_choice` (M4.1) read and save the machine and dialect chosen last, also small JSON; `remembered_settings` also gives the pane layout, which `remember_layout` saves and `reset_layout` forgets (M4.2).

**Remembered settings (M4.1).** `settings.json` in the app's configuration folder (next to `machines`) holds the machine and dialect the user chose last, read and written by `app_backend::settings`. A user machine is kept by its file name in the machine folder, never a full path, so an edited file cannot point the app at a file elsewhere. Only the user's own choices are saved, not a fallback at start-up. At start-up the UI uses the remembered machine and dialect if they are still listed and usable; otherwise the first usable machine and the first dialect, with a note in the status line. A missing settings file gives the defaults; a damaged one gives what could be read, with a note, never an error. Settings the app does not know (from a later version) are kept when it saves.

**Remembered layout (M4.2).** The same file holds the pane layout and the window. The UI saves the panes (the editor's share of the width, the diagnostics panel's share of the right column, the maximised pane) when a drag ends or a pane is maximised, and restores them at start-up, clamped so every pane keeps its minimum size in the window as it is now. The shell owns the window: it starts hidden (`tauri.conf.json`), is given its remembered size (logical pixels), place (physical desktop pixels) and maximised state, fitted to the screens there are now by `settings::place_window` (shrunk to the screen, moved back onto it, and left to the desktop to place when its screen is gone), and then shown. The shell follows resizes and moves and saves the window's normal size and place, and whether it is maximised, when it closes. Under Wayland the desktop places windows, so no position is saved or set there; only the size and maximised state. Whether the colour key is folded and which diagnostics filters are ticked are kept in the same layout and saved when clicked (M4.2a). **Reset layout** in the toolbar goes back to the default panes, colour key, filters and window size and forgets the saved layout. All of this uses Tauri's own window API (no plugin), from Rust, so the window needs no extra permissions.

The user's machine folder is `machines` in the app's configuration folder. On start, if it has no `*.toml` files, the app copies in the shipped mill and router examples (built into the app, so nothing is downloaded; an existing file is never overwritten). `list_machines` lists only the folder's files; the shipped examples are listed instead only when the folder has none (for example, it could not be written).

The window shows the UI built from `ui/` into `ui/dist`, which Tauri embeds at compile time (so the UI is built before the shell, locally and in CI). Interpretation and file reads run on a worker thread (`spawn_blocking`). The commands answer only the app's own pages: a page from another origin gets "not allowed" (tested), and the content security policy allows no remote content. It allows inline *styles* (`style-src 'unsafe-inline'`), because the editor (CodeMirror) injects its styles at run time; scripts stay `'self'` only. The UI takes file paths from the native open and save dialogs (`tauri-plugin-dialog`, M2.8b), so the app knows the open file's real path and can save over it; the files themselves are read and written by the app's own commands. Those commands accept any path the window sends: the app does not itself check that a path came from a dialog. Inside the Flatpak the sandbox limits them to the files the user picked; a build outside it relies on the window being trustworthy (no remote content, scripts `'self'` only). Having the Rust side open the dialogs and accept only the paths it handed out is on the wishlist. The window may use only those dialogs, the app's commands, and the close-request event (to ask before closing with unsaved changes); the dialog plugin's file-system companion (`tauri-plugin-fs`, a dependency) is not enabled. Saving writes UTF-8 with the file's own line endings, through a temporary file that then replaces the original: it writes through a symlink to the real file, keeps the file's permissions where the file system allows, and never follows a leftover temporary file (M3 review).

### 8.5 Large programs

Interpretation runs off the UI thread (Rust thread on desktop/mobile; Web Worker if the core is ever compiled to WASM). Results could stream in chunks (`RenderBuffer` per chunk) with progress events; the header's `flags` reserve a bit for "partial / more follows".

**Measured in M3.9** (`cargo run --release -p bench`, a generated program of 1 000 000 blocks, 24 MB): interpretation about 2 s and the render buffer 0.3 s (61 MB) in the cloud test container; on the owner's desktop (Flatpak) the file opens in 1–2 s, orbiting and zooming are smooth, line clicks answer at once and the editor stays smooth. **Chunking is not needed** for that size, so it stays unbuilt. **Memory is the weak point:** the core peaks at about 715 MB while interpreting (the owner's app used about 850 MB in all): the syntax tree of every line (about 400 MB, kept because subprogram calls jump around the file) and the segments (about 300 bytes each). **Vertex budget (M3 review).** Huge arcs at a fine tolerance need many points each (a 100 m full circle at 0.01 mm needs about 70 000), so a small crafted file could exhaust memory. The app therefore draws at most `ir::render::MAX_VERTICES` (10 million) vertices: `fit_tolerance` doubles the arc tolerance until the drawing fits (or every arc is at its minimum of one point per 90°), and an info note (`DrawingSimplified`) says so. `tessellation_len` counts points without making them. Ordinary programs, the million-block benchmark included, never reach it. That is acceptable on a desktop; before mobile (M5) the likely fixes are a smaller syntax tree (numbers stored inline rather than as a full expression per word) and a struct-of-arrays segment list (§6.6).

### 8.6 The UI (as built in M2)

Plain TypeScript (no UI framework), built with Vite into `ui/dist`, which the shell embeds. It holds **no G-code meaning**: it draws and summarises what the core produced, and sends user actions back. The parts that compute are pure modules, tested with Vitest without a browser or GPU; the DOM and three.js code around them is thin.

| Module | What it does |
|---|---|
| `src/buffer/decode.ts` | Checks and decodes the render buffer (§8.4) |
| `src/viewport/` | `toolpath.ts` turns the buffer into index lists, colours, event markers and a vertex → segment map; `views.ts` holds the standard views, orthographic fitting, and `orbitAbout` (turning the view about a node, M3.4); `viewport.ts` is the three.js view (orthographic camera, Z up, orbit/pan/zoom, picking with a pixel tolerance, highlight, hovering); `snap.ts` lists the move end points and finds the one nearest the pointer on screen (projected once per camera change); `marker.ts` shapes the tool marker, drawn as an SVG overlay so moving it never redraws the toolpath; `readout.ts` says where the marker is (position, tool, feed, line); `measure.ts` works out a measurement between two end points (M3.3), drawn on the same overlay |
| `src/viewport/` (more) | `LEGEND` in `toolpath.ts` says what each colour means; the view shows it folded into a corner. `middle-click.ts` stops the middle button (zoom) from pasting the Linux primary selection |
| `src/editor/` | CodeMirror 6: highlighting by token type, diagnostics in the gutter, and `lines.ts` (line ↔ segment lookup, for one line or a range of lines as a view of the index; the whole lines a selection covers; `latestOnly` drops results of runs overtaken by a newer one) |
| `src/freshness.ts` | Whether what is shown matches the text: edits mark it out of date (red border, **Refresh (F5)**, line sync off) until the user refreshes. Loading a file or changing the machine or dialect runs at once |
| `src/panels/` | The diagnostics panel, and the toolpath statistics (lengths, bounds, per-tool figures, counts, and the time estimate added up from the core's per-segment times), worked out from the render buffer only when the user opens the statistics dialog, since they take time on a very large program |
| `src/layout/` | `panes.ts` (pure): a pane's share of the window when a splitter is dragged, kept above a minimum size, and which pane is maximised, and a remembered share fitted to the window now (M4.2); `layout.ts` wires the splitters and maximising to the page (double-click the view or the diagnostics list; a button on the editor, where double-click selects a word) |
| `src/files.ts` | Decoding files, keeping their line endings, and opening by path without failing silently |
| `src/machines-dialog.ts` | Shows the user's machine folder and reloads the machine list |
| `src/remembered.ts` | Which machine and dialect to start with: the remembered ones if still usable, otherwise the first, with a note saying why (M4.1) |
| `src/ipc/backend.ts` | Typed calls to the shell's commands (§8.4a) |
| `src/main.ts` | Lays out the toolbar, editor, viewport, panel and status line, and wires them together |

**The editor selection in the view (M4.3).** The lines selected in the editor (whole lines; the cursor's line when nothing is selected) are drawn bright yellow on top of the toolpath, every run of a subprogram line included. The segments come from the line index as one slice (`segmentsInLines`), and `highlightGeometry` (in `toolpath.ts`) builds the drawing in two passes over typed arrays, so selecting a whole million-block program takes about a quarter of a second and nothing is copied segment by segment. While a selection is dragged, it is redrawn at most once a frame. Out of date (after an edit), nothing is highlighted. Yellow is also the tool marker's and the measure line's colour; they keep their shapes and are drawn on top in the overlay. The colour key lists "Selected".

Colours are given in sRGB and converted to linear light for the renderer. Positions shown to the user are machine coordinates until program coordinates arrive (M3.5).

### 8.7 Desktop distribution

- **Linux:** a single `.flatpak` file that users download and install (`packaging/flatpak/`; built offline from vendored crates on the GNOME runtime). Flathub is not planned. CI builds it when the app, UI, core or packaging change (and on every push to `main`) and keeps it as a download for 30 days. The app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which avoids a WebKit protocol error under Wayland on NVIDIA.
- **Windows and macOS:** not packaged yet; the app builds from source with `cargo run` in `app/shell`.
- The app id is `com.ncsoftbyai.gcodeeditsim`; the icon (`packaging/icons/`) is a placeholder. On Linux the app gives its windows that id (GTK's program name) before any opens, so desktops match a window to `com.ncsoftbyai.gcodeeditsim.desktop` and show its icon; a test keeps the identifier, desktop file, icon and Flatpak manifest in step.

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

    /// M codes the machine file lists in `ignore_codes` (M3.0). The
    /// interpreter removes them from every block, with one note per code,
    /// before the dialect sees it. M codes only: a G code's axis words
    /// would be left behind and read as a move.
    fn ignored_codes(&self) -> &[String] { &[] }

    /// Lathe-style X diameter handling etc. is applied here, not in the dialect.
    fn normalize_axis_value(&self, axis: usize, value: f64) -> f64 { value }
}
```

`machine-mill` implements the trivial case. `machine-lathe` is a stub (M2) with two linear axes, x (radius) and z (along the spindle). It does diameter programming through `normalize_axis_value`: a `diameter_mode` axis is halved, for absolute and incremental values alike, and machine coordinates (travel, reference, work offsets) are radius values. Arc-centre offsets and `R` are not converted, so they are radius values.

**What the lathe stub showed about the traits (M2.2).** A Fanuc program ran on the lathe through the unchanged `Machine`, `Dialect` and interpreter interfaces, including diameter moves, ZX-plane arcs, limits, reference returns and kinematics. The problems it found are recorded here and not fixed yet:

- **The dialect's assumed power-on plane depends on the machine.** The Fanuc dialect assumes G17 (XY) when a program sets no plane. A lathe has no Y axis, so an arc without G18 has no plane and is rejected (with an error, never drawn wrongly). A lathe dialect would assume G18. The power-on modal state may need the machine's say, e.g. a default plane in `AxisSet`. To be decided with the first real lathe dialect (M8).
- **Diameter mode is fixed per axis.** `normalize_axis_value` has no program state, so a control whose programs can switch between diameter and radius input cannot be modelled through it. If one is needed, the dialect would convert instead, or the trait would take a mode.
- **Choosing the machine type from `family` cannot live in `machine-api`,** which may not depend on concrete machines. It lives in the app: `app_backend::load_machine` (M2.3).
- **The tool direction is a guess.** `ToolPose::direction` is fixed per machine type (a lathe tool points +x). Real tool orientation comes with tool geometry (M6). `tool_model` (tool geometry and the tool table) is deferred to M6.

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

Not started; planned for M7, after tool geometry (M6). Dexels are the leading candidate for milling (good speed/memory, handles 3-axis well); revisit for 5-axis.

## 11. Testing strategy

- **Golden tests** (`tests/golden/`): each `*.nc` has an expected result file per stage: `<name>.syntax.txt` (parse result) and `<name>.ir.json` (interpreted with the Fanuc dialect on the generic 3-axis mill). The IR file is a compact summary built by `testkit` (end points by axis letter, arc centres and radii, feed, tool, flags, diagnostics, bounding box), rounded to 1e-6, rather than the full `Segment` structs, so diffs stay small and human-checkable. Regenerating goldens requires an explicit flag (`UPDATE_GOLDEN=1`) and a reviewed diff.
- **Property tests** (`proptest`): parser never panics on arbitrary bytes; arc tessellation stays within tolerance; incremental/absolute equivalence.
- **Cross-boundary test:** the §8.4 `.bin` fixture decoded in Rust and TS.
- **Dialect conformance:** one test file per modal-group behaviour (e.g. "G91 persists across blocks", "G83 peck depths").
- **Fuzzing** (`cargo-fuzz`) on the lexer/parser from the start. G-code files come from untrusted sources. CI runs the parser target for 60 seconds on every push and PR. It is seeded with the golden programs and uses a pinned nightly and cargo-fuzz version. Any panic, stack overflow, input taking over 10 s, or memory use over 2 GB fails the job, and the crashing input is uploaded. Longer runs are done locally.

## 12. Known risks and how we handle them

| Risk | Mitigation |
|---|---|
| Traits designed from one implementation are wrong in ways only a second one reveals | Done in M2.2: a stub `machine-lathe` and a stub Osai 10 Series dialect. Findings are in §7.2 and §9; one interface change (`ArcCenter::Position`). |
| Fanuc behaviour varies by series, options and parameters | Target one documented profile first; surface everything unsupported as a Diagnostic; keep parameters in the dialect TOML. |
| Cutter compensation (G41/G42) is genuinely hard | Deferred to M6. Until then a `Warning` diagnostic is emitted and the path is drawn without compensation. |
| Test programs written by the agent, not a CAM system | A private corpus of the owner's real programs, run locally with `tools/corpus` (M3.0; clean again at M3.10); agreed excerpts become golden tests. More programs from other CAM systems and controls are welcome. |
| Large programs, and crafted files, exhausting memory | A million blocks needs about 715 MB in the core (§8.5; to reduce before M5). The drawing has a vertex budget, so huge arcs in a small file are drawn coarser rather than exhausting memory (§8.5). |
| The window could be misused to read or write any file | No remote content, scripts `'self'` only, the commands answer only the app's own pages (§8.4a); the Flatpak sandbox limits files to those the user picked. Rust-side dialogs are on the wishlist. |
| Tauri mobile maturity | **Android: done (M2.3 spike).** The app's own commands ran on a real arm64 phone: the program crossed as a raw request body and the render buffer came back as raw bytes, identical in every checked value to the desktop build, and file save/load round-tripped raw bytes. Branch `claude/android-spike`, `app/shell/SPIKE.md`. **iOS: not yet tried** (needs a Mac); planned much later. |
| Heidenhain conversational / non-G-code formats | Parser crate is isolated; a non-G-code front end can produce the same `Command` stream. Not in scope yet. |
| `f32` GPU precision | Origin-relative vertices (§8.2). |

## 13. Milestones

Status: M1, M2 and M3 done; M4 next (plan agreed).

1. **M1: Core skeleton** *(done)*. Workspace, CI, `gcode-syntax`, `ir`, Fanuc subset (G0/G1/G2/G3, G17–19, G20/21, G90/91, G54–59, F/S/T/M basics), `machine-mill`, interpreter, golden tests. No UI.
2. **M2: Render buffer + minimal UI** *(done)*. Binary boundary, three.js viewport, CodeMirror editor, line ↔ segment sync, stats and diagnostics panel. Desktop only. Add stub lathe/second dialect to stress the traits. Added along the way: an Android spike of the binary boundary (passed), a Flatpak package (§8.7), CI jobs that run only when their files change, refresh on demand after edits, and the tool marker with hover readout (M3.1/M3.2 brought forward). See `docs/plans/M2_PLAN.md`.
3. **M3: A dependable backplotter** *(done)*. Real programs draw without false alarms: ignorable codes (tool height and wear compensation, machine-specific codes) give an info note instead of an error; codes that change later moves are ignored with a warning, never hiding moves; subprograms; per-tool statistics and a simple time estimate; large programs. Backplot tools: tool marker, hover info on the nearest endpoint, measuring between two points, orbit around a snapped point. Coordinate shifts and program coordinates (M3.5) and canned cycles (M3.6) are on hold, left to the community; macro variables are on the wishlist. Added along the way: resizable and maximisable panes, a colour key, the machine folder seeded with the examples, the window's app id for the desktop icon. See `docs/plans/M3_PLAN.md`.
4. **M4: Code transformations** *(next)*. Translate, rotate and scale (one factor for X and Y) a selected range of program lines; the core rewrites only the words that must change (keeping comments and number style) and the editor applies it as one undoable edit. Also: remembering the machine, dialect and window layout between sessions, the editor selection shown bright yellow in the 3D view, a better icon, and `.nc` files opening in the app from the Linux file manager. See `docs/plans/M4_PLAN.md`.
5. **M5: Mobile.** The desktop app on Android (the M2.3 spike passed); decide on the mobile layout. iOS later. Reduce the core's memory for large programs first (§8.5: about 715 MB for a million blocks).
6. **M6: Machine state and accuracy.** Tool table and tool geometry, cutter radius compensation, time estimation with an acceleration model, the dog-leg rapid path, speed-limit checks.
7. **M7: Material removal (mill).**
8. **M8+: Additional dialects (Haas, then others), the full Osai 10 Series, and machine types (lathe, mill-turn).**

The order was revised on 2026-10-31 (owner decision): the editor and backplotter are matured (M3) and code transformations added (M4) before mobile and simulation. Tool length (height) and wear compensation are not planned: a virtual machine shows the path as programmed, and these codes are ignored with an info note.

## 14. Disclaimer

This software visualizes and analyzes G-code. It is not a safety verification tool. Always verify programs independently before running them on a machine.
