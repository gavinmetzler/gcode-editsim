# Development status

A hand-over document for anyone, human or AI agent, continuing this project:
fixing bugs, finishing M2, or starting later milestones. Read it
after `CLAUDE.md` and `ARCHITECTURE.md`; it records *what exists and why*,
the decisions made along the way, and what is still open.

ARCHITECTURE.md remains the source of truth for design. If this file and
ARCHITECTURE.md disagree, ARCHITECTURE.md wins: fix this file.

Last updated: M3.0 (real-program corpus, lenient handling of ignorable codes).
M2 is planned in [`docs/plans/M2_PLAN.md`](docs/plans/M2_PLAN.md), M3 in
[`docs/plans/M3_PLAN.md`](docs/plans/M3_PLAN.md); the milestone order is in
ARCHITECTURE.md §13.

---

## 1. Where things stand

**M1 (core skeleton) is complete, with known limitations** (section 6).
**M2 (render buffer and minimal desktop UI) is complete** (M2.8b, the
last step, adds opening and saving through the native dialogs and the
machine folder). **Next: M3, a dependable backplotter**
(`docs/plans/M3_PLAN.md`); M3.1 and M3.2 are already done.

The app is usable today as an editor beside a synced 3D backplot of Fanuc
milling programs, with diagnostics and statistics, that opens and saves
files.

| Area | Status |
|---|---|
| Tests | 494 pass (`cargo test --workspace`); 10 in the app shell (`cd app/shell && cargo test`); 85 in the UI (`cd ui && npm test`) |
| CI (green on GitHub) | fmt, clippy (`-D warnings`), dependency rules (`tools/depcheck`), tests, a 60 s parser fuzz job, a Tauri app-shell job (WebKit installed; fmt, clippy, tests), a UI job (lint, typecheck, test, build) and a Flatpak build (artifact kept 30 days); on pull requests each job runs only when its files change (`tools/ci`) |
| UI | Editor (M2.6: CodeMirror, highlighting, diagnostics in the gutter; since M2.8a edits mark the view out of date until the user refreshes) beside the toolpath viewport (M2.5: three.js, orthographic, standard views); line ↔ segment sync both ways; a tool marker that follows the pointer, with a readout of position, tool and feed (M3.1/M3.2, brought forward); measuring between two nodes by dragging (M3.3); orbiting about a node (M3.4); a colour key in the view; a diagnostics panel under the viewport and statistics on demand in a dialog (M2.7); opening and saving through the native dialogs, keeping line endings, with a prompt before losing unsaved changes (M2.8b); served by the app shell (embedded `ui/dist`) |
| Packaging | Linux: a single `.flatpak` file (`packaging/flatpak/`), tested on the owner's desktop; not on Flathub. Windows and macOS: build from source |
| Mobile | Android spike passed on a real phone (branch `claude/android-spike`, `app/shell/SPIKE.md`); iOS not tried |
| Licence | AGPL-3.0-only (`LICENSE`; `license` field in every Cargo manifest) |

**What the core does:** a Fanuc milling program plus a machine
definition file produce IR segments in machine coordinates (the Osai 10
Series stub covers a small subset). That covers:

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
- **Branch:** all work happens on the session's designated branch (M1:
  `claude/hopeful-euler-b7gaff`; M2: `claude/affectionate-goodall-mi4cum`). After a PR merges, restart the branch from
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
  explicit instruction (CLAUDE.md rule 9). The owner approved each change
  so far one at a time (the UI and Tauri jobs, the Flatpak job, the
  per-area path filters); ask again for anything else.
- **Check requests against the plan before building** (owner, 2026-10-31).
  If a request is already planned for a later step, or not planned at
  all, say so and ask where it belongs before writing any code. Ideas
  that are not planned yet go in `docs/WISHLIST.md`.
- **Private material:** the Osai 10 Series manual is in the owner's
  private repository `gcode-editsim-manuals`. Cite it by title and page
  with short quotes only; never copy it into this repository. The same
  goes for the owner's real programs (M3.0).

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
| `core/dialect-osai` | Osai 10 Series milling **stub** (M2.2) | `data/osai.toml`; `src/block.rs`; three-letter commands in `src/three_letter.rs`; notes in `docs/dialect-notes/osai.md` |
| `core/dialect-fanuc` | Fanuc milling | `data/fanuc.toml` (75 codes with handlers); `src/block.rs` (all block logic); `FanucOptions { decimal_input }` |
| `core/machine-api` | Machine trait and definition-file loader | `MachineDef::from_toml_str` (rejects unknown settings); `AxisDef` (letter, kind, travel, arc_offset_letter, direction, rapid_rate, reference, diameter_mode); `Machine` trait; `check_limits`; `work_offset(id)` |
| `core/machine-mill` | 3-axis mill | `Mill::new(def)`, `Mill::generic()`; example file `data/generic_3axis_mill.toml` |
| `core/machine-lathe` | 2-axis lathe **stub** (M2.2) | `Lathe::new(def)`, `Lathe::generic()`; X diameter → radius in `normalize_axis_value`; example file `data/generic_2axis_lathe.toml` |
| `core/interpreter` | Dialect + machine to IR | `interpret(source, file, &dyn Dialect, &dyn Machine, Options) -> ir::Program`; `examples/summary.rs` |
| `core/testkit` | Golden helpers (no file access) | `summary(&Program, letters)`: compact JSON rounded to 1e-6; `compare`; `UPDATE_GOLDEN` |
| `core/sim` | Empty placeholder (M7) | |
| `app/backend` | The app's behaviour without Tauri (M2.3a) | `load_machine` (by `family`), `dialects`/`load_dialect`, `interpret_to_buffer`, `files::{read_program, save_program, list_machine_files}` |
| `app/shell` | The Tauri 2 desktop shell (M2.3b), **its own Cargo workspace** | Commands in `src/lib.rs` (`interpret` raw body → raw render buffer, `load_file`, `save_file`, machine and dialect choice); IPC tests on Tauri's mock runtime in `tests/ipc.rs`; the UI (`ui/dist`) embedded at compile time |
| `tools/gen-ts` | Generates `ui/src/generated/render_buffer.ts` from `core/ir` | `cargo run -p gen-ts`; its test fails if the committed file is stale |
| `ui/` | TypeScript UI (M2.4–M2.7; ARCHITECTURE.md §8.6) | Vite 8, TypeScript 6 (`strict`), ESLint 10 + typescript-eslint strict type-checked, Vitest 5; `src/buffer/decode.ts` (render buffer decoder); `tests/decode.test.ts` (cross-boundary fixture test); `src/viewport/` (three.js viewport); `src/editor/` (CodeMirror editor, line ↔ segment mapping); `src/panels/` (diagnostics panel, statistics dialog, M2.7); `src/dom.ts` (element helper); `src/ipc/backend.ts` (typed calls to the app's commands) |
| `packaging/flatpak` | Flatpak packaging (single `.flatpak` file, installed by users; not on Flathub) | `build.sh`; manifest builds offline (UI built and crates vendored beforehand); sandbox: window and GPU only, files via the portal; **placeholder icon** |
| `tools/corpus` | Runs real programs (kept private) through the pipeline and reports diagnostics per file (M3.0) | `cargo run -p corpus -- --dialect fanuc FILE...` |
| `tools/ci` | Which CI jobs a pull request needs | `changed-areas.sh` (rules), `test-changed-areas.sh` (its tests, run in CI first). Unknown paths run everything; docs-only changes run nothing; pushes to `main` run everything |
| `tools/depcheck` | The dependency-rule checker | `cargo run -p depcheck` |

User-facing documentation:
- `docs/machine-definition.md`: the machine file format.
- `docs/dialect-notes/fanuc.md`, `docs/dialect-notes/osai.md`: dialect behaviour and its sources.
- `docs/plans/M2_PLAN.md`, `docs/plans/M3_PLAN.md`: milestone plans.
- `packaging/flatpak/README.md`: building and installing the Flatpak.
- `docs/WISHLIST.md`: ideas kept for later, not yet planned.
- `tests/golden/README.md`: the golden-file formats.

---

## 4. Decisions that shape later work

The reasons are in the PR descriptions and in ARCHITECTURE.md.
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
- `ArcCenter::Position` (M2.2): a centre programmed as a point (absolute
  in work coordinates, or incremental from the start). Its values go
  through `normalize_axis_value`, unlike `ArcCenter::Offset`.
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
- **Never hide moves** (owner decision, 2026-10-31; ARCHITECTURE.md
  §7.3a): G92, G52, G10, G65, G68, G51, G51.1, G16, M98, M99 and unknown G
  codes give one diagnostic, their words are data, and later moves are
  drawn as programmed. Canned cycles draw their hole positions as rapids
  in the plane. (The M1 motion blockers are gone.)
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
- `family = "mill"` or `"lathe"` selects the machine type; the app looks
  it up (`app_backend::load_machine`).
- On a lathe, `diameter_mode` X values are halved; machine coordinates are
  radius. What the lathe stub found about the traits is in ARCHITECTURE.md
  §9 (assumed G17 plane on a machine without Y; fixed diameter mode;
  where the `family` lookup lives; tool direction).

**App and UI (M2)**
- The shell (`app/shell`) is its own Cargo workspace, so the core builds
  and tests without WebKit. All behaviour is in `app/backend`.
- Bulk data is raw bytes both ways: the program as the request body, the
  render buffer as the reply (`tauri::ipc::Response`). Machine, dialect
  and options are set beforehand and kept in the app's state.
- The commands refuse remote origins; the content security policy allows
  inline styles (CodeMirror needs them) but only the app's own scripts.
- Plain TypeScript, no UI framework. Pure, tested modules do the work;
  DOM and three.js code stays thin (ARCHITECTURE.md §8.6).
- The viewport is orthographic with Z always up; Top is tilted by 1e-4
  rad, because orbit controls cannot look exactly along their up axis.
- Colours are sRGB converted to linear light.
- The editor's text is what is interpreted, so line numbers always agree.
  Edits do not re-run the program (M2.8a, owner's request): they mark the
  results out of date with a red border and a **Refresh (F5)** button,
  and line sync is off until the refresh, since lines may have moved.
  Loading a file or changing the machine or dialect runs at once. A run
  overtaken by a newer one is dropped. Automatic updates may come back as
  a setting (`docs/WISHLIST.md`).
- Statistics are worked out in the UI from the render buffer, only when
  the user asks (a dialog, owner's request: on demand, all in one view); the units
  the program used are not in the buffer, so they are not shown.
- Files are opened and saved through `tauri-plugin-dialog` (M2.8b): the
  UI gets a real path from the native dialog and the app's own
  `load_file`/`save_file` commands do the reading and writing. Saving
  keeps the file's line endings; a file that was not UTF-8 asks before
  saving, since its unreadable bytes would change.
- Flatpak: single file, offline build, `WEBKIT_DISABLE_DMABUF_RENDERER=1`
  (fixes a Wayland protocol error on NVIDIA, confirmed by the owner).

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
| I2: G43 tool length not applied (Z and Z limits off by the tool length) | Not planned (owner decision 2026-10-31): ignored with an info note, M3.0 |
| I3: rapids drawn straight; Fanuc default is non-linear "dog-leg" (param 1401#1 LRP, unverified) | Open, M6 |
| I4: IR lacks stop/pause, coolant, spindle direction, program end | **Fixed** (M2.0): event segments |
| I6: max feed / max rpm / rapid rates loaded but unused | Open |
| I7: about 19 Fanuc behaviours marked *(unverified)* | Open; needs the owner's control manual |
| Minor: R arc with equal end points is a warning in the dialect but an error in the interpreter | Open |
| Minor: G95 feed is stored as mm/min (wrong for cycle time) | Open, M3.8 |
| Minor: no warning for a feed move with the spindle stopped | Open (feature) |
| Minor: undefined work offset uses `InvalidValue`; "after program end" uses `NotYetSimulated`; limit summary points at line 0 | Open |

**The biggest remaining risk:** every test program was written by the
agent. No real CAM output has been run through the pipeline yet. The owner
was asked to try a few real programs with the `summary` example.

---

## 6. Known limitations (also listed in the README)

- G43 is not applied, and Z limit checks ignore the tool length (by
  design: height and wear compensation are not planned).
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
- Positions are shown in machine coordinates (M3.5).
- One real dialect (Fanuc) and one real machine type (3-axis mill). The
  Osai 10 Series dialect and the lathe are stubs; what they found is in
  ARCHITECTURE.md §7.2 and §9.

---

## 7. Practical lessons (avoid repeating these)

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

**UI and app (M2)**
- **Check the UI in a real browser, not only with unit tests.** Washed-out
  colours (sRGB vs linear), invisible dashes, a misleading perspective
  top view and unstyled CodeMirror (blocked by the content security
  policy) were all found by looking. Headless Chromium with the app's IPC
  mocked by a real render buffer works in the container.
- **Flatpak in a container** needs a D-Bus session (and a system bus),
  the portals and `XDG_RUNTIME_DIR` started by hand. `XDG_RUNTIME_DIR`
  must be `/run/user/<uid>`: the document portal returns paths under it,
  and Flatpak mounts the sandbox's copy there, so any other folder makes
  every portal path "No such file or directory" inside the sandbox; CI needs `elfutils` and
  `librsvg2-common`, and `--disable-rofiles-fuse`.
- **Disk space is limited** in the cloud container: the shell's `target/`,
  the Android SDK and Gradle caches fill it quickly.

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

Decided (2026-10-31), roadmap revision:
- **Mature the editor and backplotter before mobile or simulation.** New
  order: M3 a dependable backplotter, M4 code transformations, M5
  mobile, M6 machine state and accuracy, M7 material removal, M8+ more
  dialects and machine types (ARCHITECTURE.md §13).
- **Ignorable codes:** machine-specific codes, and tool height and wear
  compensation, are ignored with an info note, not an error. Rarely used
  codes (turret switches, some coordinate changes) may be ignored where
  the backplot stays useful.
- **Wanted early:** toolpath statistics, and translating, scaling and
  rotating a selected part of the program (M4), ahead of mobile and
  simulation.
- **Backplot tools in M3:** a tool marker (a thick vertical line with an
  arrowhead, deliberately unlike the M7 tool with size and shape), hover
  info on the nearest endpoint (position, tool, feed), measuring between
  two points, and orbiting around a snapped point.
- **Agent proposals the owner did not object to:** G92, G52, G10 and Osai
  origins are implemented rather than ignored (ignoring a shift misplaces
  later moves); still-unsupported coordinate changes are drawn
  approximate instead of hidden; program coordinates are the default
  display.
- **Flatpak:** distributed as a single `.flatpak` file; Flathub is not
  planned.

Decided (2026-10-31), after M3.4:
- **Coordinate shifts and program coordinates (M3.5) and canned cycles
  (M3.6) are on hold**, left to the community (`docs/WISHLIST.md`). Work
  offsets and origins are used in ways only the machine explains (on the
  owner's gang router, G54-G56 select spindles), and CAM output rarely
  needs canned cycles.
- **Never hide moves.** A backplot drawn without an unsupported code's
  effect, with a warning, is more useful than none.

Decided (2026-10-31), after M3.0:
- **G codes that carry coordinates are implemented, never ignored.**
  Ignoring one leaves its axis words behind to be read as a move (found
  with `ignore_codes`, which therefore takes M codes only). G92 is the one
  such code not implemented yet: until M3.5 it is skipped with a warning,
  its axis words consumed as data so they are never a move.

Still open:
1. Can the owner verify the *(unverified)* Fanuc items against their
   control's manual?
2. The Osai 10 Series manual is in the owner's private repository
   `gcode-editsim-manuals` (not in this one, for copyright reasons).
   Items marked *(unverified)* in `docs/dialect-notes/osai.md` need a
   real control.
3. A private corpus of the owner's real Fanuc and Osai programs (M3.0),
   and the exact `(UAO,n)` message the owner saw.
