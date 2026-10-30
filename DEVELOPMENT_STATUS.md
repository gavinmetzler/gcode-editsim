# Development status

A hand-over document for anyone, human or AI agent, continuing this project:
fixing M1 bugs, extending its scope, or starting later milestones. Read it
after `CLAUDE.md` and `ARCHITECTURE.md`; it records *what exists and why*,
the decisions made along the way, and what is still open.

ARCHITECTURE.md remains the source of truth for design. If this file and
ARCHITECTURE.md disagree, ARCHITECTURE.md wins: fix this file.

Last updated: M2.2, part 1 (lathe stub). M2 is planned in
[`docs/plans/M2_PLAN.md`](docs/plans/M2_PLAN.md).

---

## 1. Where things stand

**M1 (core skeleton) is complete, with known limitations** (section 6).

| Area | Status |
|---|---|
| Tests | 327 pass (`cargo test --workspace`); 36 in `ir` with `--all-features` |
| CI (green on GitHub) | fmt, clippy (`-D warnings`), dependency rules (`tools/depcheck`), tests, and a 60 s parser fuzz job |
| UI | none yet; M2 starts it |
| Licence | AGPL-3.0-only (`LICENSE`; `license` field in every Cargo manifest) |

**What works headlessly:** a Fanuc milling program plus a machine
definition file produce IR segments in machine coordinates. That covers:

- work offsets G54–G59
- G28 and G53
- exact arcs: I/J/K, R, full circles and helices
- travel-limit checks, including arc bulges
- a diagnostic for everything unsupported

Try it:
```bash
cargo run -p interpreter --example summary -- tests/golden/basic_mill.nc [machine.toml]
```

---

## 2. Working agreement with the owner

- **The owner** (GitHub `gavinmetzler`) reviews and merges every PR. Work
  arrives one step at a time ("do step N only").
- **Branch:** all work happens on the session's designated branch (so far
  `claude/hopeful-euler-b7gaff`). After a PR merges, restart the branch from
  `origin/main` (`git checkout -B <branch> origin/main`) and open a **new**
  PR. Never stack on merged history.
- **Force-push is blocked by the environment.** Fix mistakes with a new
  commit, not by amending.
- **PR descriptions** are in plain English and cover: what was built,
  decisions the owner should know about, what is deliberately not handled,
  how it was tested, and a dependency table (CLAUDE.md rule 7).
- **The owner prefers plain-English explanations** in chat. Ask before
  large or ambiguous work; CLAUDE.md requires a plan first for large tasks.
- **Be conservative about CNC correctness.** Never claim Fanuc behaviour
  is correct unless it is tested *and* sourced. Mark everything else
  *(unverified)* in `docs/dialect-notes/fanuc.md`. The same applies to
  Osai (`docs/dialect-notes/osai.md`).
- **Workflow and CI files** (`.github/workflows/`) change only with
  explicit instruction (CLAUDE.md rule 9). The owner allowed the M2 UI
  and Tauri CI jobs as a one-off; ask again for anything else.

---

## 3. Map of the code

All core crates have `#![forbid(unsafe_code)]`, never panic on input, and
build for `wasm32-unknown-unknown`. Allowed dependencies between crates are
enforced by `tools/depcheck` (table in `tools/depcheck/rules.toml`, mirrored
in ARCHITECTURE.md §5).

| Crate | What it does | Key items |
|---|---|---|
| `core/gcode-syntax` | Bytes to blocks (one per non-blank line). Knows no G-code meaning. | `parse(&[u8]) -> Program`; `Block { line, block_skip, sequence_number, words, assignments, comments, program_delimiter, errors }`; `Expr` tree (never evaluated); `MAX_DEPTH` = 64. Fuzz target in `fuzz/` |
| `core/ir` | The neutral output data model | `Pos = [f64; 9]`; `SegKind` (values fixed for the render buffer); `Segment`; `ArcGeom { center, plane: ArcPlane { first, second }, radius, turns }`; `DiagCode` (u16 values fixed); `Diagnostics`; `segments_for_line`; `bounding_box`; `tessellate`; `render::encode_render_buffer` + `render::layout`; `serde` feature |
| `core/dialect-api` | What every dialect shares | `Dialect` trait (`initial_state`, `interpret_block(block, &BlockContext { axes, src }, state, out, diag)`); `Command`; `ModalState` (incl. `motion_blockers`, `first_report`); `CodeTable` (TOML); `AxisSet`; `CodeKey` (G1 = G01 = G1.0) |
| `core/dialect-fanuc` | Fanuc milling | `data/fanuc.toml` (75 codes with handlers); `src/block.rs` (all block logic); `FanucOptions { decimal_input }` |
| `core/machine-api` | Machine trait and definition-file loader | `MachineDef::from_toml_str` (rejects unknown settings); `AxisDef` (letter, kind, travel, arc_offset_letter, direction, rapid_rate, reference, diameter_mode); `Machine` trait; `check_limits`; `work_offset(id)` |
| `core/machine-mill` | 3-axis mill | `Mill::new(def)`, `Mill::generic()`; example file `data/generic_3axis_mill.toml` |
| `core/machine-lathe` | 2-axis lathe **stub** (M2.2) | `Lathe::new(def)`, `Lathe::generic()`; X diameter → radius in `normalize_axis_value`; example file `data/generic_2axis_lathe.toml` |
| `core/interpreter` | Dialect + machine to IR | `interpret(source, file, &dyn Dialect, &dyn Machine, Options) -> ir::Program`; `examples/summary.rs` |
| `core/testkit` | Golden helpers (no file access) | `summary(&Program, letters)`: compact JSON rounded to 1e-6; `compare`; `UPDATE_GOLDEN` |
| `core/sim` | Empty placeholder (M6) | |
| `tools/depcheck` | The dependency-rule checker | `cargo run -p depcheck` |

User-facing documentation:
- `docs/machine-definition.md`: the machine file format.
- `docs/dialect-notes/fanuc.md`: Fanuc behaviour and its sources.
- `tests/golden/README.md`: the golden-file formats.

---

## 4. Decisions that shape later work

The reasons are in the PR descriptions (#2–#11) and in ARCHITECTURE.md.
Changing any of these is a deliberate, documented change.

**Syntax**
- A line with a syntax error is **incomplete**: words before the error
  are kept, the rest is dropped, and later stages skip the block.
- `X10` and `X10.` stay distinct (`Number::decimal_point`).
- `;` starts a comment that runs to the end of the line.
- Blank lines produce no block, but line numbers still count them.
- Spans are 0-based byte offsets within the line.

**IR**
- The arc plane is a pair of axis indices, not an XY/ZX/YZ enum.
  `helical_delta` was removed.
- A full circle is an arc whose end points coincide exactly. The
  interpreter snaps ends within 1e-6 mm to the start, because float drift
  once collapsed G2 full circles.
- Positions are **machine coordinates**: work offsets are applied by the
  interpreter.
- **Render buffer v1** (M2.1): `ir::encode_render_buffer(program, tol,
  to_xyz)`, layout exactly as ARCHITECTURE.md §8.3. Origin = centre of
  the vertices' bounding box. The caller passes the arc tolerance and the
  axis → x/y/z mapping (`machine.forward_kinematics(p).position`). Fixture
  `tests/golden/render_buffer_v1.bin`. Single channel only in v1.
- **Events are zero-length segments** (M2.0): stops, spindle, coolant
  and program end each have their own `SegKind` (7–15), one segment per
  command, in program order. Events while the position is unknown get
  `SegFlags::POSITION_UNKNOWN` (bit 2). Kind and flag values are fixed.

**Dialect API**
- Feed, spindle speed and spindle state are separate commands. So are tool
  select and tool change. `Stop` exists.
- Commands carry no `SourceRef`: the caller attaches the block's reference.
- Axis words use indices into the `AxisSet`, never letters. The arc-offset
  letter (I for X) comes from the **machine file**.
- Macros: literal numbers only. `#var` or `[...]` in a word gives an error
  and the block is skipped; an assignment gives a warning.

**Fanuc dialect**
- No-decimal numbers use the least input increment by default (DPI = 0:
  `X10` = 0.010 mm), with a one-time warning; `DecimalInput::Calculator`
  is the alternative. The owner chose to keep this Fanuc default.
- Group 1/2/3/6 modes are **not** set at power-on. The first time a
  program relies on one, there is an "assumed …" warning and G00, G17,
  G90 or G21 is used.
- The last G code of a modal group in a block wins, with a warning.
- Effects within a block happen in this order: G modes (units first), then
  F/S/T, then M3/M4/M6/M8, then the motion, then M0/M1/M2/M5/M9/M30.
- A rejected block has **no** effect at all, because it is validated
  before anything is applied.
- **Motion blockers** (ARCHITECTURE.md §7.3a): G92, G52, G10, G65, G68,
  G51, G51.1, G16, M98, M99 and unknown G codes hide later motion until
  their cancel code, or until the end of the program.
- Canned cycles hide motion until G80 or G00–G03.
- Hidden motion emits `Command::PositionUnknown`.
- G28 (via `reference` in the machine file) and G53 are simulated. G30
  leaves the axes' position unknown.

**Interpreter**
- The machine starts at its reference position.
- Nothing is drawn from an unknown position until every axis has been
  given an absolute value again (one warning per episode).
- Arc radius tolerance is 0.01 mm. A rejected arc does not move the
  machine.
- Limit warnings are capped at 50; segments are still flagged.
- Blocks after M2/M30 are not run (one info note).
- Block delete (`Options::block_delete`) is off by default.

**Machine files**
- TOML; always in mm.
- Work offsets are numbered 1, 2, … (Fanuc G54 = 1); 0 means machine
  coordinates.
- `family = "mill"` or `"lathe"` selects the machine type. The caller picks
  the type today; the app (M2.3) will look it up from `family`.
- On a lathe, `diameter_mode` X values are halved; machine coordinates are
  radius. What the lathe stub found about the traits is in ARCHITECTURE.md
  §9 (assumed G17 plane on a machine without Y; fixed diameter mode;
  where the `family` lookup lives; tool direction).

**Golden files**
- `<name>.syntax.txt` (parser) and `<name>.ir.json` (pipeline, Fanuc on
  the generic mill).
- Regenerate with `UPDATE_GOLDEN=1 cargo test -p gcode-syntax --test golden`
  or `-p interpreter`, then review the diff by hand.

---

## 5. M1 review: findings and status

An M1 review against ARCHITECTURE.md focused on CNC correctness. It was a
self-review: the same agent wrote the code.

| Finding | Status |
|---|---|
| C1: unsupported codes silently corrupted later moves | **Fixed** (#9): motion blockers |
| C2: refused moves left the position stale (G28 tool-change idiom) | **Fixed** (#9): G28/G53 simulated, unknown-position tracking |
| C3: G2 full circle collapsed by float drift | **Fixed** (#9): snapping, plus the equivalence property test |
| I1: canned cycle not cancelled by G00–G03 | **Fixed** (#9) |
| I5: power-on defaults assumed silently | **Fixed** (#9): "assumed" warnings |
| I8: CI did not enforce the dependency rule; fuzz not in CI; no abs/inc test | **Fixed** (#9, #10) |
| I2: G43 tool length not applied (Z and Z limits off by the tool length) | Open, M4 |
| I3: rapids drawn straight; Fanuc default is non-linear "dog-leg" (param 1401#1 LRP, unverified) | Open, M5 |
| I4: IR lacks stop/pause, coolant, spindle direction, program end | **Fixed** (M2.0): event segments |
| I6: max feed / max rpm / rapid rates loaded but unused | Open |
| I7: about 19 Fanuc behaviours marked *(unverified)* | Open; needs the owner's control manual |
| Minor: R arc with equal end points is a warning in the dialect but an error in the interpreter | Open |
| Minor: G95 feed is stored as mm/min (wrong for cycle time) | Open, M5 |
| Minor: no warning for a feed move with the spindle stopped | Open (feature) |
| Minor: undefined work offset uses `InvalidValue`; "after program end" uses `NotYetSimulated`; limit summary points at line 0 | Open |

**The biggest remaining risk:** every test program was written by the
agent. No real CAM output has been run through the pipeline yet. The owner
was asked to try a few real programs with the `summary` example.

---

## 6. Known limitations (also listed in the README)

- G43 is not applied, and Z limit checks ignore the tool length.
- Rapids are straight lines.
- Speed limits are unused.
- Not supported:
  - macros beyond literals
  - subprograms
  - canned cycles
  - cutter compensation
  - G30 positions
  - coordinate rotation, scaling, mirroring and polar input
  - rotary axes on the mill
- One dialect (Fanuc), one real machine type (3-axis mill); the lathe is a
  stub.

---

## 7. Practical lessons from M1 (avoid repeating these)

**Writing tests**
- **Start bug fixes with a failing test** (CLAUDE.md). It caught real
  mistakes several times, including a G95 regression in #9.
- **Wrong expectations are common.** Test expectations were wrong more
  often than the code (arc centre side, move counts, limits in machine
  coordinates). Derive expected values carefully and check them by hand.
- **Mutation-check important tests.** Temporarily break the code and
  confirm the tests fail. Delete any `*.proptest-regressions` file this
  creates before committing.
- **The golden summary rounds to 1e-6, which hides float problems.** Use
  unrounded checks (per-segment `bounding_box`) for geometry.

**Editing files**
- **Python heredoc edits eat a single `\` at the end of a line.** That
  broke a Rust string continuation. Use `\\` in Python strings. The golden
  test now rejects messages containing double spaces.
- **`cargo fmt` reformats code,** so exact-match edits made after it can
  fail. Re-read the file first.

**Test helpers**
- **The "assumed …" warnings are filtered out** in the `dialect-fanuc`
  and `interpreter` helpers (`tests/common/mod.rs`), so short test programs
  stay readable. Check `Run::assumed` / `run_unfiltered` when testing them.

**Tooling**
- **Fuzzing needs nightly:**
  `cargo +nightly-2026-10-29 fuzz run parser` from `core/gcode-syntax/`
  (cargo-fuzz 0.13.2). The fuzz crate is its own workspace.
- **CI pins** ubuntu-24.04, checkout@v7, upload-artifact@v7, the Rust
  1.97.0 toolchain, the nightly and cargo-fuzz. Bump them deliberately, in
  their own PR.

---

## 8. Owner decisions and open questions

Decided (2026-10-30):
- Licence: **AGPL-3.0-only**.
- CLAUDE.md "Current milestone" is now M2 (the agent updated it).
- Decimal-point input: **keep the Fanuc default** (DPI = 0).
- **No** scheduled long fuzz run. The 60 s CI job stays. A
  whole-pipeline fuzz target is not planned.
- M2 sequencing: see `docs/plans/M2_PLAN.md` section 2. The second
  dialect is **Osai 10 Series**.

Left to the agent's judgement, and decided as follows:
- GitHub Actions stay on the pinned major tags (no SHA pinning or
  Dependabot for now). Changing this means editing workflows, which needs
  the owner's permission.
- CI stays on Ubuntu 24.04 until a tool needs newer packages.

Still open:
1. Can the owner verify the *(unverified)* Fanuc items against their
   control's manual?
2. Does the owner have the Osai 10 Series programming manual? The Osai
   stub depends on it.
