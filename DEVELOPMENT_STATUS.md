# Development status

A hand-over document for anyone, human or AI agent, continuing this project:
fixing bugs or starting later milestones. Read it
after `CLAUDE.md` and `ARCHITECTURE.md`; it records *what exists and why*,
the decisions made along the way, and what is still open.

ARCHITECTURE.md remains the source of truth for design. If this file and
ARCHITECTURE.md disagree, ARCHITECTURE.md wins: fix this file.

Last updated: M5.7a-c (Linux, Windows and macOS packages, the release workflow, version 0.5.0, 2026-11-03).
Each milestone has a plan in `docs/plans/` (M2 to M4 complete, M5 agreed
and in progress, [`docs/plans/M5_PLAN.md`](docs/plans/M5_PLAN.md)); the milestone
order is in ARCHITECTURE.md §13.

---

## 1. Where things stand

**M1 (core skeleton) is complete, with known limitations** (section 6).
**M2 (render buffer and minimal desktop UI) is complete.**
**M3 (a dependable backplotter) is complete** (`docs/plans/M3_PLAN.md`):
the owner's real Fanuc and Osai programs draw with no errors and every gap
explained; tool marker, hover, measuring and orbiting about a node;
subprograms; statistics per tool with a simple time estimate; a million
blocks opens in a second or two on the owner's desktop. Coordinate shifts
and program coordinates (M3.5) and canned cycles (M3.6) are on hold, left
to the community.
**M4 (code transformations) is complete** (`docs/plans/M4_PLAN.md`):
translate, rotate and scale selected lines of Fanuc and Osai programs,
rewriting only the words that must change, applied as one undoable edit;
the machine, dialect, layout, colour key and diagnostics filters
remembered; the editor selection shown bright yellow in the view; a new
icon; `.nc` files opening from the Linux file manager (Flatpak).

**Next: M5, everyday use, Osai level with Fanuc, and packages**
(`docs/plans/M5_PLAN.md`). The owner agreed the plan and its order
(2026-11-02); M5.1-M5.6 are done (M5.6: every "not supported"
message on the owner's programs gone, 2026-11-03), M5.7 (packages) is
next. The milestones were
renumbered on 2026-11-02 (section 8): mobile is now M6, machine state
M7, material removal M8, more dialects and machine types M9+.

The app is usable today as an editor beside a synced 3D backplot of Fanuc
(and, as a stub, Osai 10 Series) milling programs, with diagnostics,
statistics and a time estimate, that opens and saves files and transforms
selected lines.

| Area | Status |
|---|---|
| Tests | 712 pass (`cargo test --workspace`); 31 in the app shell (`cd app/shell && cargo test`); 148 in the UI (`cd ui && npm test`) |
| CI (green on GitHub) | fmt, clippy (`-D warnings`), dependency rules (`tools/depcheck`), tests, a 60 s parser fuzz job, a Tauri app-shell job (WebKit installed; fmt, clippy, tests), a UI job (lint, typecheck, test, build), a Flatpak build (artifact kept 30 days), the packages (Linux `.deb` and `.rpm`, the Windows installer, the macOS `.dmg`; each installed or started) when their settings change (`packages.yml`, also run by the release) and a dependency audit (`cargo audit` on the three Rust lock files, `npm audit` on the UI; on pull requests only when a lock file changes); on pull requests each job runs only when its files change (`tools/ci`). **Releases:** pushing a tag `vX.Y.Z` matching the version in `app/shell/tauri.conf.json` and `app/shell/Cargo.toml` runs `release.yml`, which publishes a GitHub Release with the packages under fixed names (M5.7) |
| UI | Editor (M2.6: CodeMirror, highlighting, diagnostics in the gutter; since M2.8a edits mark the view out of date until the user refreshes) beside the toolpath viewport (M2.5: three.js, orthographic, standard views); line ↔ segment sync both ways; a tool marker that follows the pointer, with a readout of position, tool and feed (M3.1/M3.2, brought forward); measuring between two nodes by dragging (M3.3); orbiting about a node (M3.4); a colour key in the view; panes resized by dragging and maximised by double-click; the editor margin follows the diagnostics filters; a diagnostics panel under the viewport and statistics on demand in a dialog (M2.7), with per-tool times and a time estimate (M3.8); opening and saving through the native dialogs, keeping line endings, with a prompt before losing unsaved changes (M2.8b); served by the app shell (embedded `ui/dist`) |
| Packaging | Linux: a single `.flatpak` file (`packaging/flatpak/`), tested on the owner's desktop; not on Flathub. The machine folder is created on first start with the example mill and router files; the window reports the app id, so the desktop shows its icon. Windows and macOS: build from source |
| Mobile | Android spike passed on a real phone (branch `claude/android-spike`, `app/shell/SPIKE.md`); iOS not tried |
| Licence | AGPL-3.0-only (`LICENSE`; `license` field in every Cargo manifest) |

**What the core does:** a Fanuc milling program plus a machine
definition file produce IR segments in machine coordinates (the Osai 10
Series stub covers a small subset). That covers:

- work offsets G54–G59
- G28 and G53
- exact arcs: I/J/K, R, full circles and helices
- subprograms (M98/M99 within the file)
- feed per minute and per revolution (G94/G95)
- travel-limit checks, including arc bulges
- an estimated time per segment (no acceleration)
- a diagnostic for everything unsupported, and machine-specific codes
  noted once and skipped

Try it:
```bash
cargo run -p interpreter --example summary -- tests/golden/basic_mill.nc [machine.toml]
```

---

## 2. Working agreement with the owner

- **The owner** (GitHub `gavinmetzler`) reviews and merges every PR. Work
  arrives one step at a time ("do step N only").
- **Branch:** all work happens on the session's designated branch (M1:
  `claude/hopeful-euler-b7gaff`; M2 and M3: `claude/affectionate-goodall-mi4cum`; M4: `claude/lucid-tesla-9bjqjk`; a new session gets its own). After a PR merges, restart the branch from
  `origin/main` (`git checkout -B <branch> origin/main`) and open a **new**
  PR. Never stack on merged history.
- **One open PR at a time.** The designated branch carries one PR; work
  for the next step waits until it merges (prepare it locally on another
  local branch if useful, then cherry-pick onto the restarted branch).
  Pushing early adds it to the open PR.
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
  per-area path filters, and after the M3 review the dependency audit
  and a time limit on the shell job's package install); ask again for
  anything else.
- **Check requests against the plan before building** (owner, 2026-10-31).
  If a request is already planned for a later step, or not planned at
  all, say so and ask where it belongs before writing any code. Ideas
  that are not planned yet go in `docs/WISHLIST.md`.
- **How the owner tests:** on Linux (KDE Plasma, Wayland, Dolphin), with
  the Flatpak from the CI run on `main` after a merge, and reports back.
  Ask them to check what the container cannot (Wayland, their GPU,
  their real programs).
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
| `core/ir` | The neutral output data model | `Pos = [f64; 9]`; `SegKind` (values fixed for the render buffer); `Segment`; `ArcGeom { center, plane: ArcPlane { first, second }, radius, turns }`; `DiagCode` (u16 values fixed); `Diagnostics`; `Segment::time` (estimated seconds, M3.8); `segments_for_line`; `bounding_box`; `tessellate`; `tessellation_len`; `path_length`; `render::encode_render_buffer` + `render::layout`; `render::fit_tolerance` and `MAX_VERTICES` (vertex budget); `serde` feature |
| `core/dialect-api` | What every dialect shares | `Dialect` trait (`initial_state`, `interpret_block(block, &BlockContext { axes, src }, state, out, diag)`); `Command`; `ModalState` (incl. `feed_mode`, `first_report`); `CodeTable` (TOML); `AxisSet`; `CodeKey` (G1 = G01 = G1.0) |
| `core/dialect-osai` | Osai 10 Series milling **stub** (M2.2) | `data/osai.toml`; `src/block.rs`; three-letter commands in `src/three_letter.rs`; notes in `docs/dialect-notes/osai.md` |
| `core/dialect-fanuc` | Fanuc milling | `data/fanuc.toml` (75 codes with handlers); `src/block.rs` (all block logic); `FanucOptions { decimal_input }` |
| `core/machine-api` | Machine trait and definition-file loader | `MachineDef::from_toml_str` (rejects unknown settings); `AxisDef` (letter, kind, travel, arc_offset_letter, direction, rapid_rate, reference, diameter_mode); `Machine` trait; `check_limits`; `work_offset(id)` |
| `core/machine-mill` | 3-axis mill | `Mill::new(def)`, `Mill::generic()`; example files `data/generic_3axis_mill.toml` and `data/generic_3axis_router.toml` (no travel limits, every work offset at zero) |
| `core/machine-lathe` | 2-axis lathe **stub** (M2.2) | `Lathe::new(def)`, `Lathe::generic()`; X diameter → radius in `normalize_axis_value`; example file `data/generic_2axis_lathe.toml` |
| `core/interpreter` | Dialect + machine to IR | `interpret(source, file, &dyn Dialect, &dyn Machine, Options) -> ir::Program`; `interpret_traced(…, lines) -> (Program, Vec<BlockTrace>)` (M4.5: per run of each block in the lines, state after it, commands, errors, position before/after in work coordinates, mm); `examples/summary.rs` |
| `core/transform` | Rewrites selected lines exactly (M4) | `TextEdit { line, span, text }`; `apply_edits` (every other byte kept); `source_lines` (split as the parser splits); `rewrite_word` (one word's new value in the program's style; no edit if it reads back the same); `transform(…, lines, &Transformation)` (Translate M4.6, Rotate M4.7, Scale M4.8) → `Transformed { edits, notes }` or `TransformError::Refused { line, reason }` (M4.6). Number style: `gcode_syntax::NumberStyle`; meaning: `Dialect::read_value` / `format_value` (Fanuc in `dialect-fanuc/src/numbers.rs`) |
| `core/testkit` | Golden helpers (no file access) | `summary(&Program, letters)`: compact JSON rounded to 1e-6; `compare`; `UPDATE_GOLDEN` |
| `core/sim` | Empty placeholder (M8) | |
| `app/backend` | The app's behaviour without Tauri (M2.3a); `transform::{transform_program, program_point, encode_edits, decode_edits}` (M4.9) | `load_machine` (by `family`), `dialects`/`load_dialect`, `interpret_to_buffer`, `files::{read_program, save_program, seed_machine_folder, list_machine_files}`, `settings::{load, save, update, RememberedMachine}` (the remembered machine and dialect, M4.1) |
| `app/shell` | The Tauri 2 desktop shell (M2.3b), **its own Cargo workspace** | Commands in `src/lib.rs` (`interpret` raw body → raw render buffer, `load_file`, `save_file`, machine and dialect choice); IPC tests on Tauri's mock runtime in `tests/ipc.rs`; the UI (`ui/dist`) embedded at compile time |
| `tools/gen-ts` | Generates `ui/src/generated/render_buffer.ts` from `core/ir` | `cargo run -p gen-ts`; its test fails if the committed file is stale |
| `ui/` | TypeScript UI (M2.4–M2.7; ARCHITECTURE.md §8.6) | Vite 8, TypeScript 6 (`strict`), ESLint 10 + typescript-eslint strict type-checked, Vitest 5; `src/buffer/decode.ts` (render buffer decoder); `tests/decode.test.ts` (cross-boundary fixture test); `src/viewport/` (three.js viewport); `src/editor/` (CodeMirror editor, line ↔ segment mapping); `src/panels/` (diagnostics panel, statistics dialog, M2.7); `src/dom.ts` (element helper); `src/ipc/backend.ts` (typed calls to the app's commands) |
| `packaging/flatpak` | Flatpak packaging (single `.flatpak` file, installed by users; not on Flathub) | `build.sh`; manifest builds offline (UI built and crates vendored beforehand); sandbox: window and GPU only, files via the portal; the icon (M4.10) is `packaging/icons/<id>.svg`, its PNGs rendered from it |
| `tools/bench` | Times interpretation and the render buffer on a generated large program (default 1 000 000 blocks) or a given file, with peak memory (M3.9) | `cargo run --release -p bench -- [--blocks N] [--write FILE] [PROGRAM]` |
| `tools/corpus` | Runs real programs (kept private) through the pipeline and reports diagnostics per file (M3.0) | `cargo run -p corpus -- --dialect fanuc FILE...` |
| `tools/ci` | Which CI jobs a pull request needs | `changed-areas.sh` (rules), `test-changed-areas.sh` (its tests, run in CI first). Unknown paths run everything; docs-only changes run nothing; pushes to `main` run everything |
| `tools/depcheck` | The dependency-rule checker | `cargo run -p depcheck` |

User-facing documentation:
- `docs/machine-definition.md`: the machine file format.
- `docs/dialect-notes/fanuc.md`, `docs/dialect-notes/osai.md`: dialect behaviour and its sources.
- `docs/plans/M2_PLAN.md`, `docs/plans/M3_PLAN.md`, `docs/plans/M4_PLAN.md` (complete), `docs/plans/M5_PLAN.md` (agreed, in progress): milestone plans.
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
- **Render buffer** (v1 in M2.1, v2 in M3.8 adds each segment's
  estimated time): `ir::encode_render_buffer(program, tol,
  to_xyz)`, layout exactly as ARCHITECTURE.md §8.3. Origin = centre of
  the vertices' bounding box. The caller passes the arc tolerance and the
  axis → x/y/z mapping (`machine.forward_kinematics(p).position`). Fixture
  `tests/golden/render_buffer_v2.bin`. Single channel only.
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
- **Subprograms (M3.7):** the dialect numbers programs and turns M98/M99
  into call/return commands; the interpreter follows them (call stack,
  depth 10, 2,000,000-block limit). The UI's line lookup is a line
  index, since a subprogram line has segments from every call.
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
  §7.3a): G52, G10, G65, G68, G51, G51.1, G16 and unknown G codes give
  one diagnostic, their words are data, and later moves are
  drawn as programmed. Canned cycles draw their hole positions as rapids
  in the plane. (The M1 motion blockers are gone.)
- G28 (via `reference` in the machine file) and G53 are simulated. G30
  leaves the axes' position unknown.

**Transformations (M4.4)**
- Rewriting changes only a word's number (after the letter and any
  spaces); the edit is skipped when the new text reads back as the old
  value, so the identity transformation leaves any program unchanged
  (property test over generated programs and every golden program, both
  units and both DPI settings, plus a stricter check that each literal's
  own value is written back as exactly the same text, `-0` aside).
- Fanuc reads lengths and angles through `numbers.rs`, which the
  rewriter also uses, so reading and writing cannot disagree.

- **Translate (M4.6):** follows the position the rewritten program
  reaches; G91 moves inside the selection are unchanged, the first one
  gains the offset and the first incremental move after it loses it
  (owner decision); an absolute move after it is not edited (its unwritten
  axes stay moved, with a note). An arc may not start the selection or
  follow it. Refusals come from the core (errors, unknown positions, G53,
  G28/G30, rotary offsets) and from `Dialect::transform_refusal`.
  `tests/translate_paths.rs` compares the interpreted paths before and
  after (400 random programs; about two thirds transformed, the rest
  refused at an arc).

- **Rotate (M4.7):** `Transformation::Rotate { plane, angle, centre }`;
  arc offsets turn (added when no longer zero), R stays, an arc in another
  plane is refused. `tests/rotate_paths.rs` (800 random programs, about a
  third with arcs inside the selection) compares the paths within one
  increment, since turned values are rounded. Lesson: a property test
  that passes can still miss a whole case (here, every arc in a rotated
  selection was refused because of rounding); count what it exercises and
  mutation-check it.

- **Scale (M4.8):** `Transformation::Scale { plane, factor, centre }`;
  I/J and R scale (`Dialect::arc_radius_letter` names R); a factor of
  zero or less, and an arc in another plane, are refused.
  `tests/scale_paths.rs` (800 random programs). The shared generator
  (`tests/common`) also writes R arcs (not near a half circle, where R is
  ill-conditioned); arc centres are compared within 0.005 mm, since the
  interpreter works them out from rounded values.

- **Osai transformations (M4.9a):** the Osai stub reads and writes numbers
  (`dialect-osai/src/numbers.rs`, through `dialect_api::format_as_written`),
  names R and I/J (`arc_centre_letters`), and refuses its own codes;
  absolute centre positions move like end points (`tests/osai.rs`,
  `tests/osai_paths.rs`; the shared generator writes Osai programs too).
  The move after the selection is adjusted only if it runs once.
- **Real programs (M4.9a, locally):** translate, rotate and scale over
  several ranges of the owner's Osai and Fanuc programs: within 0.0005 mm
  in the selection; outside it only the documented case (an absolute move
  after the selection leaves the axes it does not write moved). Whole-file
  selections are refused at the first G79/G92 line.
- **Real programs (M4.12, locally):** all six of the owner's programs,
  translate, rotate 30° and 90°, scale 1.5 and 0.8, on the longest
  stretches between refusals (up to about 19 700 lines): the path keeps
  its shape within 0.001 mm and nothing outside the selection moves,
  except two cases left to the operator (owner, 2026-11-02): selecting a
  subprogram's body without its calls changes every call, and lines at
  the start of a selection without axis words (a tool change) keep the
  position from before the selection. A throwaway test in
  `core/transform/tests/` (deleted afterwards) did this; comparing in
  machine coordinates needs a reference point per work offset, since
  rotating and scaling happen in program coordinates.

**Interpreter**
- `interpret_traced` (M4.5) traces only the lines asked for; both positions use the work offset in effect *for* the block, so `G55 X10.` has a `before` already in G55 coordinates.
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
  the program used are not in the buffer, so they are not shown. Times
  come from the core: the interpreter sets each segment's estimated time
  (feed moves at their feed capped at `max_feed`, rapids at the slowest
  axis's rapid rate, dwells, the machine's `tool_change_time`), carried in
  the render buffer (v2, M3.8); the UI only adds them up.
- Files are opened and saved through `tauri-plugin-dialog` (M2.8b): the
  UI gets a real path from the native dialog and the app's own
  `load_file`/`save_file` commands do the reading and writing. Saving
  keeps the file's line endings; a file that was not UTF-8 asks before
  saving, since its unreadable bytes would change. Saving writes through
  symlinks and keeps the file's permissions (M3 review).
- The drawing has a vertex budget (10 million): past it, arcs are drawn
  coarser, with a `DrawingSimplified` note (M3 review).
- **Remembered settings (M4.1):** `settings.json` next to the machine
  folder holds the machine (a shipped id, or a file *name* in the machine
  folder, never a path) and the dialect. Saved only when the user picks
  one (`remember_choice`), so a fallback at start-up never overwrites
  it. Reading never fails; settings a later version adds are kept on
  save. The start-up choice and its status-line note are worked out in
  `ui/src/remembered.ts`.
- **Remembered layout (M4.2):** the same file holds the panes (saved by
  the UI after a drag or a maximise) and the window (saved by the shell
  when it closes; restored before the hidden window is shown, fitted to
  the screens by `settings::place_window`). No window position under
  Wayland. The colour key's folding and the diagnostics filters are in
  the same layout (M4.2a, added by the owner). **Reset layout** in the
  toolbar goes back to the defaults.
  Checked for real under Xvfb with openbox (`xdotool`, `wmctrl -c` to
  close politely); there the window manager decides the frame, so the
  client area sits a few pixels inside the saved frame position.
- **Editor selection in the view (M4.3):** the selected lines (whole
  lines; the cursor's line otherwise) are drawn bright yellow, replacing
  the white one-line highlight; the range comes from
  `LineIndex.segmentsInLines` (a view of the index). Since M5.2 a shader
  draws it from each vertex's line (`ui/src/viewport/highlight.ts`), so
  any selection costs two numbers and a redraw.
- **Transform dialog (M4.9):** **Transform…** (Ctrl+T) on the selected
  lines; the core's edits come back as a binary edit list (`GCTE`,
  fixture `tests/golden/transform_edits_v1.bin`) and are applied as one
  isolated undo step; byte spans are turned into editor (UTF-16) positions
  in `ui/src/transform/edits.ts`. Values are mm (inches: wishlist). The
  centre can be picked in the view (`program_point` gives its program
  coordinates; refused while the view is out of date; Esc cancels).
- **G-code files on the desktop (M4.11):** MIME type `text/x-cnc-gcode`
  (`packaging/flatpak/<id>.xml`), `Exec=gcode-editsim %f`; the app opens a
  file from its command line (`startup_file`). KDE's Qt lets types and
  globs in Flatpak's exported folder hide those in `/usr/share`, so the
  package also defines NetCDF and the Neo Geo ROM type in full;
  `packaging/flatpak/check-mime.sh` checks GIO and Qt (`apt install
  libglib2.0-bin python3-pyqt6 python3-pyqt6.sip`; PyQt6 is built for the
  system's python3.12).
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
| I3: rapids drawn straight; Fanuc default is non-linear "dog-leg" (param 1401#1 LRP, unverified) | Open, M7 |
| I4: IR lacks stop/pause, coolant, spindle direction, program end | **Fixed** (M2.0): event segments |
| I6: max feed / max rpm / rapid rates loaded but unused | Partly fixed (M3.8): rapid rates and max feed are used by the time estimate; max rpm is still unused |
| I7: about 19 Fanuc behaviours marked *(unverified)* | Open; needs the owner's control manual |
| Minor: R arc with equal end points is a warning in the dialect but an error in the interpreter | **Fixed** (M3 review): a warning in both, an arc of 0° |
| Minor: G95 feed is stored as mm/min (wrong for cycle time) | **Fixed** (M3.8): G95 is feed per revolution, F × S |
| Minor: no warning for a feed move with the spindle stopped | Open (feature) |
| Minor: undefined work offset uses `InvalidValue`; "after program end" uses `NotYetSimulated`; limit summary points at line 0 | Open |

**Real programs (M3.0, again at M3.10):** the owner's private corpus
(four Fanuc router programs, two Osai 10 Series programs) draws with no
errors; every warning or note names a real gap. Run it with
`tools/corpus`; the programs stay out of this repository.

---

## 6. Known limitations (also listed in the README)

- G43 is not applied, and Z limit checks ignore the tool length (by
  design: height and wear compensation are not planned).
- Rapids are straight lines.
- Speed limits are not checked (max feed and rapid rates are used only by
  the time estimate); the time estimate has no acceleration (M7).
- Fanuc G04 (dwell) is an error and adds no time (owner decision,
  M3.8); Osai dwells have no time (`DWT` is not read).
- A million blocks needs about 715 MB in the core (850 MB in the app);
  to be reduced before mobile (ARCHITECTURE.md §8.5).
- Not supported:
  - macros beyond literals (wishlist)
  - subprograms in other files (calls within the file work, M3.7)
  - canned cycles (on hold: hole positions only)
  - cutter compensation
  - G30 positions
  - coordinate shifts, rotation, scaling, mirroring and polar input (on
    hold: ignored with a warning)
  - rotary axes on the mill
- Positions are shown in machine coordinates (program coordinates are on
  hold).
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
- **An icon or metadata change can break only the Flatpak** (M4.10): the
  SVG's `<svg` tag must come within its first 256 bytes, or gdk-pixbuf, and
  so `appstreamcli compose`, cannot tell it is an SVG (a long comment
  before it did that). Test packaging changes with `appstreamcli compose`
  on the build tree, or a full `build.sh`, and look at CI on `main` after a
  merge.
- **Flatpak in a container** needs a D-Bus session (and a system bus),
  the portals and `XDG_RUNTIME_DIR` started by hand. `XDG_RUNTIME_DIR`
  must be `/run/user/<uid>`: the document portal returns paths under it,
  and Flatpak mounts the sandbox's copy there, so any other folder makes
  every portal path "No such file or directory" inside the sandbox; CI needs `elfutils` and
  `librsvg2-common`, and `--disable-rofiles-fuse`.
- **Opening a file through the installed Flatpak (M4.11):** start Xvfb,
  a system bus (`mkdir -p /run/dbus && dbus-daemon --system --fork`) and a
  session bus (`dbus-launch`), `/usr/libexec/xdg-document-portal`
  (package `xdg-desktop-portal`, needs `/dev/fuse`), set
  `XDG_RUNTIME_DIR=/run/user/0` and put
  `~/.local/share/flatpak/exports/share` first in `XDG_DATA_DIRS`, then
  `gio launch <exports>/applications/<id>.desktop FILE`. Without the
  system bus, `flatpak run` fails with "Could not connect: No such file
  or directory".
- **Linux packages** (M5.7a): build them with Tauri's CLI from npm
  (`packaging/linux/README.md`; `cargo install tauri-cli` builds the same
  tool in about 10 minutes). The Windows and macOS packages cannot be
  built here, but `rustup target add x86_64-pc-windows-msvc` and `cargo
  clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings` in
  `app/shell` check the Windows code (macOS needs Apple's tools).
  Install the `.deb` with `apt-get install ./file.deb` and start it
  through its menu entry under Xvfb (`gio launch
  /usr/share/applications/gcode-editsim.desktop FILE.nc`). The container
  lacks `desktop-file-utils`, so `gio mime` shows no default app until it
  is installed (every desktop has it). `rpm2cpio` exits 1 on Tauri's
  `.rpm` files although it writes them out whole (rpm 4.18), and the
  `.rpm` cannot be installed here (Ubuntu); `check-packages.sh` reads it.
- **Disk space is limited** in the cloud container: the shell's `target/`,
  the Android SDK and Gradle caches fill it quickly.

**Testing the UI and app in the container** (scratch scripts live in
`/tmp` and are lost with the container; rebuild them from this recipe)
- **A real render buffer:** a throwaway Cargo binary in `/tmp` with a
  path dependency on `app/backend`, calling
  `app_backend::interpret_to_buffer(program, name,
  app_backend::files::shipped_machine("generic_3axis_mill"), "fanuc",
  Default::default())` and writing the bytes to a file.
- **Serve the UI:** `cd ui && npm ci && npm run build`, then start
  `npx vite preview --port 4173 --strictPort` detached
  (`setsid nohup … &`).
- **Drive it with Playwright** (pre-installed: `require('playwright')`,
  Chromium with `--use-gl=swiftshader --enable-unsafe-swiftshader`). In
  `page.addInitScript`, set `globalThis.isTauri = true` and
  `window.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label:
  'main' } }, transformCallback: () => 1, invoke: async (cmd) => … }`,
  answering `list_dialects`, `list_machines`, `select_machine`,
  `set_run_options`, `plugin:event|listen` (return 1),
  `plugin:dialog|open`, `load_file` and `interpret` (return the buffer
  as an `ArrayBuffer`). The page root is itself a `<main id="app">`, so
  the pane layout is `main main`.
- **Headless Chromium is not WebKitGTK** (M5.1): the out-of-date outline
  was hidden under the WebGL canvas in the app but showed in Chromium. For
  layering, compositing or anything else that looks right in Chromium but
  not to the owner, run the real app under Xvfb (below; building the shell
  needs `libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
  librsvg2-dev`, and `xdotool` drives it) and screenshot it with `import
  -window root`. Draw things that must sit above the canvas as positioned
  boxes, not outlines.
- **Measuring UI speed with a held key** (M5.2): send the repeats with
  CDP `Input.dispatchKeyEvent` (`rawKeyDown`, `autoRepeat: true`) about
  every 33 ms without awaiting them, so they queue like a real held key;
  time from the key-up until every event is handled; count WebGL draw
  calls and frames by wrapping `drawArrays`/`drawElements` and a
  `requestAnimationFrame` loop. With a million segments, compositing the
  canvas in software makes every frame slow here whatever the code does,
  so also run with `.viewport canvas { display: none }` to see the
  JavaScript and editor cost alone.
- **Holding a key in the real app** (M5.2): `xdotool key --repeat` sends
  separate presses and releases, not a held key (the page sees no repeat
  flag). Use `xdotool keydown Next; sleep 3; xdotool keyup Next` (with
  `keydown Shift_L` around it for Shift): the X server repeats the key as
  a keyboard does. Time what the user sees: screenshot the gutter and
  status bar (`import -window root -crop …`) until they stop changing. CPU
  use is no guide here: the idle app uses about 100% while the editor has
  focus (software rendering), and runs vary two-fold, so repeat each
  measurement three or four times. Chromium did not show the Shift+Page
  Down problem at all.
- **Ask the owner for the diagnostics report** (M5.2): Ctrl+Shift+D in
  the app, **Copy**, paste. It shows how long key presses waited (the
  app falling behind), the cost of following the cursor and of each 3D
  draw, the frame interval, the graphics cards and drivers, the Flatpak
  runtime and the WebKitGTK settings. WebKit reports every GPU to WebGL as
  "Apple GPU", so the real card comes from the app side
  (`/sys/class/drm`). Open it in the real app here too: it works under
  Xvfb, and `xclip -o -selection clipboard` reads what Copy put there.
- **What the owner's reports showed (M5.2, 2026-11-02):** KDE Wayland, an
  RTX 4090 beside an Intel iGPU, device pixel ratio 2, Flatpak runtime
  GNOME 51, NVIDIA 580 with its Flatpak GL extension. **Under Wayland,
  GTK gives every repeat of a held key the first press's timestamp**, so
  "Key press waiting" there was only the time since the key went down
  (it misled M5.2d into "the queue is before the page"). The real
  figures: 99 presses sent in about 3 s reached the page over 23.3 s
  (one every 235 ms) while the page spent 24 ms on each, and a held Down
  arrow was much better with the 3D view hidden. So each key event costs
  about 200 ms to get on screen outside the page's code (frames copied
  through the CPU at device pixel ratio 2); M5.2e paces held cursor keys
  in the app instead. `WEBKIT_DISABLE_DMABUF_RENDERER=0`
  still closes the window (Wayland error 71). `GDK_BACKEND=x11` alone fails
  ("Failed to initialize GTK"), because the manifest only allows X11 as a
  fallback: test X11 with `flatpak run --nosocket=wayland --socket=x11
  --env=GDK_BACKEND=x11 com.ncsoftbyai.gcodeeditsim`: it runs but is as
  slow; adding `WEBKIT_DISABLE_DMABUF_RENDERER=0` there gives a white
  window ("Failed to create GBM buffer").
- **Time key handling by the page's clock, not the event's:** keep
  `performance.now()` deltas between presses reaching the page; an
  event's `timeStamp` is not reliable for repeats (above).
- **The Chrome test page** (M5.2): the real UI in a plain browser, to
  tell WebKitGTK problems from our own. Build the UI (`npm run build`),
  copy `ui/dist`, a generated program (`tools/bench --blocks 200000
  --write test.nc`, never one of the owner's) and its render buffer
  (the throwaway `interpret_to_buffer` binary, above) into one folder,
  and add a `mock.js` loaded before the app's script in `index.html`. It
  sets `globalThis.isTauri = true` and a `window.__TAURI_INTERNALS__`
  whose `invoke` answers `list_dialects`, `list_machines`,
  `remembered_settings`, `startup_file` (a path), `load_file` and
  `interpret` (`fetch` of the two files as `ArrayBuffer`s) and
  `plugin:event|listen`. The owner serves it with `python3 -m
  http.server 8765` and opens it in Chrome; Chrome also tells WebGL the
  real GPU. Zip it and send it; it does not go in the repository.
- **Check a drawing change pixel by pixel:** screenshot the view for the
  same selections with the old build and the new one, and compare them
  (`compare -metric AE old.png new.png null:` gives the number of pixels
  that differ). The M5.2 shader highlight matched the old one exactly.
- **Before writing a new test file, check the name is free:** `cat >`
  silently replaced the existing `ui/tests/diagnostics.test.ts` once
  (M5.2); the test count dropped, which is how it was caught.
- **Software rendering is slow here:** drawing a million segments takes
  seconds per frame in the container but is smooth on the owner's
  desktop. Judge JavaScript time with the CDP profiler, and rendering on
  the owner's machine.
- **The real app under Xvfb:** `Xvfb :98` plus `dbus-launch`, then
  `app/shell/target/debug/gcode-editsim`; `xdotool search --classname`
  reads the window class. A stale `/tmp/.X11-unix/X98` without a running
  Xvfb gives "Can't open display": remove it and start Xvfb again.
- **The owner's real programs:** clone `gavinmetzler/gcode-editsim-manuals`
  (add it to the session first) next to this repository, then
  `cargo run --release -p corpus -- --dialect fanuc|osai FILE...`.
- **Large programs:** `cargo run --release -p bench -- --blocks N
  [--write FILE]`. Timings in the container vary two- to three-fold
  between runs: run it several times.

**Tooling**
- **Don't pipe a check into `grep` inside an `&&` chain:** the chain then sees grep's exit status, not the check's (a clippy failure went unnoticed that way in M4.5). Run `cargo clippy -q … -- -D warnings` bare, or use `set -o pipefail`.
- **Chain checks with `&&`, never `;`.** Twice a failing `cargo fmt` or
  `clippy` was followed by a commit and push because the commands were
  joined with `;`.
- **`pkill -f <pattern>` can match and kill your own shell** (the
  pattern is in its command line). Use `pgrep -x`, or kill by PID.
- **Removing folders outside the repository is blocked** by a safety
  check. Keep scratch output in `/tmp` (or a folder inside the repo
  listed in `.git/info/exclude`).
- **Fuzzing needs nightly:**
  `cargo +nightly-2026-10-29 fuzz run parser` from `core/gcode-syntax/`
  (cargo-fuzz 0.13.2). The fuzz crate is its own workspace.
- **CI pins** ubuntu-24.04, checkout@v7, upload-artifact@v7, the Rust
  1.97.0 toolchain, the nightly and cargo-fuzz. Bump them deliberately, in
  their own PR. The exception is `cargo-audit`, installed at its latest
  release: an old one cannot read the current advisory database.
- **CI can hang on the runners' package mirrors** (once over 30 minutes
  on `apt-get`); the install step now times out after 10 minutes. Re-run
  a job once when that happens.

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
  dialects and machine types (ARCHITECTURE.md §13). *(Renumbered on
  2026-11-02, below: those later milestones are now M6, M7, M8 and
  M9+.)*
- **Ignorable codes:** machine-specific codes, and tool height and wear
  compensation, are ignored with an info note, not an error. Rarely used
  codes (turret switches, some coordinate changes) may be ignored where
  the backplot stays useful.
- **Wanted early:** toolpath statistics, and translating, scaling and
  rotating a selected part of the program (M4), ahead of mobile and
  simulation.
- **Backplot tools in M3:** a tool marker (a thick vertical line with an
  arrowhead, deliberately unlike the M8 tool with size and shape), hover
  info on the nearest endpoint (position, tool, feed), measuring between
  two points, and orbiting around a snapped point.
- **Agent proposals the owner did not object to** (all three superseded
  later the same day, see "after M3.4" below): G92, G52, G10 and Osai
  origins implemented rather than ignored; still-unsupported coordinate
  changes drawn approximate instead of hidden; program coordinates as the
  default display. What was built: these codes are ignored with a
  warning, later moves are drawn as programmed (not dimmed), and
  positions are machine coordinates.
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
  such code not implemented yet: with M3.5 on hold it is skipped with a
  warning, its axis words consumed as data so they are never a move.

Decided (2026-10-31 to 2026-11-01), M3.7 to M3.10:
- **Subprograms** within the file are followed; macro variables are on
  the wishlist (the owner's programs do not use them).
- **Machine files:** the examples are packaged in the app and copied
  into the user's machine folder on first start; once the folder has
  files, only they are listed (the lathe stub is not copied).
- **Time estimate:** one PR; Fanuc G04 stays an error for now; tool
  changes take an optional `tool_change_time` from the machine file.
- **Large programs:** fast enough on the owner's desktop; memory is left
  for mobile (then M5, now M6).
- **UI:** panes are resizable and maximisable (double-click the view or
  the diagnostics, a button on the editor); the editor margin follows the
  diagnostics filters; remembering the layout, with the machine and
  dialect, moved from the wishlist to the start of M4 (owner, 2026-11-01).

Decided (2026-11-01 to 2026-11-02), M4:
- **Transformations are an operator's aid,** not a fully featured
  feature for every edge case: an operator modifies a piece of a
  program and is responsible for the moves before and after it fitting
  the rest (M4_PLAN §6 item 7). So the move after the selection and the
  start of the selection are left as they are where the plan said so.
- **Osai programs are transformed too** (M4.9a, added by the owner), and
  the colour key and diagnostics filters are remembered (M4.2a).
- **The icon:** design B, an editor page beside a backplot (M4.10).

Decided (2026-11-02), at the end of M4:
- **Milestones renumbered.** A new M5 before mobile; mobile is M6,
  machine state and accuracy M7, material removal M8, more dialects and
  machine types M9+. Older plans (M2, M3) keep the old numbers as a
  record; M4_PLAN and the living documents use the new ones.
- **M5 holds the owner's list:** a tool filter (a checkbox per tool,
  right-click for one tool), Find and Replace (plain text, Ctrl+F, F3),
  packages (`.deb`, `.rpm`, Windows, macOS, the Flatpak easy to find,
  install instructions per platform), colouring by tool (rapids a dimmed
  tool colour), the hidden out-of-date border (bug), slow Page Up/Down
  (update the view about twice a second), and **Osai level with Fanuc**
  (anything that works only for Fanuc must work for Osai). The plan's
  order and its section 6 questions are for the owner to answer.
- **The scope is now mill with Fanuc and Osai** (CLAUDE.md,
  ARCHITECTURE.md §1); the lathe stays a stub.
- **Wishlist:** a timeline player (draws the toolpath over time, the
  editor cursor in step, skipping hidden tools).

Decided (2026-11-02), during M5.2 (slow Page Up/Down):
- **Keep Tauri; move to its Chromium runtime when it is stable.** On the
  owner's machine (KDE, RTX 4090, NVIDIA 580, Flatpak runtime GNOME 51)
  WebKitGTK cannot use its direct drawing path: with
  `WEBKIT_DISABLE_DMABUF_RENDERER=0` the window closes under Wayland
  (error 71) and stays white under X11 ("Failed to create GBM buffer …
  Invalid argument"), also with the Intel iGPU disabled and with
  `nvidia_drm modeset=Y`. So every frame is copied through the CPU,
  and each key event took about 235 ms to reach the screen. **The same
  UI in Chrome was "very fast"** (Chrome test page, section 7), so the
  design is sound and the problem is WebKitGTK on NVIDIA. Tauri's CEF
  runtime (`tauri-runtime-cef`) was in alpha (3.0.0-alpha.0 to alpha.6,
  September to October 2026); try it when it is stable (wishlist). The
  owner would drop macOS and Android if a different display stack were
  ever needed for NVIDIA users on Linux; with this result the roadmap is
  unchanged.
- **Newer WebKitGTK did not help (owner, 2026-11-02, after #73):** with
  `WEBKIT_DISABLE_DMABUF_RENDERER=0`, `WEBKIT_USE_SKIA_FOR_COMPOSITION=0`
  and `__NV_DISABLE_EXPLICIT_SYNC=1` (from NC1107/sink#78, WebKitGTK
  2.54 on KWin) the app started on the direct path and was fast for a
  second or two, then froze (explicit sync off); without the NVIDIA
  setting, Wayland error 71. The follow-ups suggested (pacing off with
  those settings, a newer NVIDIA driver, GNOME's nightly runtime with
  newer WebKitGTK) "didn't work" (owner; which of them were tried is not
  recorded). Building WebKitGTK ourselves was ruled out (hours of build,
  a large download, and WebKit's security updates would be ours).
- **A working combination after all (owner, 2026-11-03):**
  `WEBKIT_DISABLE_DMABUF_RENDERER=0` with only
  `__NV_DISABLE_EXPLICIT_SYNC=1` runs fast and without crashes on the
  owner's machine (adding `WEBKIT_USE_SKIA_FOR_COMPOSITION=0` crashed).
  **Decided:** the app chooses WebKit's settings at start-up (NVIDIA on
  Wayland: that combination; NVIDIA on X11: the CPU path; otherwise
  defaults), never overriding a variable the user set, and the manifest
  no longer sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` (ARCHITECTURE.md
  §8.7). Tauri's Chromium runtime stays on the wishlist.
- **Meanwhile:** the shader highlight (#70), pacing of held cursor keys
  (#72) and the diagnostics panel (Ctrl+Shift+D) stay; pacing and half
  resolution are switches in the panel.

Decided (2026-11-03), M5.6 refocused (M5_PLAN §6 item 7):
- **Diagnostics are to be about the program**, not about unsupported
  codes. Each "not supported" message goes by supporting the code or by
  listing it in the machine file's `ignore_codes`, which then silences it
  (also G codes and Osai commands: G43, UAO and DWT are silenced only when
  listed; their behaviour does not change). Unknown codes are still
  reported. CLAUDE.md's scope rule and ARCHITECTURE.md §7.3a say so.
- Cutter compensation: one warning per program. M6 with no T: a note.
  G92 (Fanuc and Osai) is supported.
- **The machine file names its dialect** (choosing the machine sets it;
  the shipped generic 3-axis router is Osai, as the owner's router is) and
  its **power-on state** (units, plane, motion mode).

Still open:
1. Can the owner verify the *(unverified)* Fanuc items against their
   control's manual?
2. The Osai 10 Series manual is in the owner's private repository
   `gcode-editsim-manuals` (not in this one, for copyright reasons).
   Items marked *(unverified)* in `docs/dialect-notes/osai.md` need a
   real control.
3. More real programs are welcome in the private corpus, especially
   from other CAM systems and controls.
4. Held Page Up/Down in the owner's Flatpak: with the app's WebKit
   setup (2026-11-03), is it smooth and stable over longer use? (Pacing
   alone, before that, was not enough.) (The M5 plan's §6
   questions were answered on 2026-11-02.)

---

## 9. Starting M5 in a new session

1. Read `CLAUDE.md`, `ARCHITECTURE.md` (§7, §8.4a, §8.6, §8.7, §13),
   this file (§2 working agreement, §7 practical lessons) and
   `docs/plans/M5_PLAN.md`.
2. **Ask the owner** to agree the M5 order and answer the plan's
   section 6 if they have not (look for an "agreed" status at the top of
   the plan). Record the answers in the plan (as M4_PLAN §6 does) in the
   M5.1 PR or a small PR of its own, and set the plan's status to agreed.
3. Work one step per PR, in order, on the session's designated branch;
   after each merge restart it from `origin/main`. Bug fixes and
   features start with a failing test.
4. Before every push, joined with `&&` and never piped into `grep`:
   `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo run -q -p depcheck`;
   when the UI changes, `cd ui && npm run lint && npm run typecheck && npm test && npm run build`;
   when the shell changes (or the UI it embeds), the same fmt, clippy
   and tests in `app/shell` (its own workspace).
5. UI work: check it in a real browser (Playwright, preinstalled), as
   §7 describes, and send the owner a screenshot; the owner tests the
   Flatpak from the CI run on `main` after each merge.
6. The owner's private programs are in `gavinmetzler/gcode-editsim-manuals`
   (`Fanuc_examples/`, `Osai10Series_examples/`): clone beside this
   repository and use them locally (`tools/corpus`, throwaway tests);
   never copy them in. They matter most for M5.6 (Osai).
7. M5.7 changes `.github/workflows/`: ask the owner before that step.
