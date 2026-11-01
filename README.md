# GCode EditSim
**A modern G-code editor, backplotter and CNC simulator for desktop and mobile.**

<img width="2265" height="1631" alt="Screenshot_20261101_072649" src="https://github.com/user-attachments/assets/737f0699-4aab-4c67-94a9-215aee960516" />


GCode EditSim is an open-source project to develop a dedicated G-code editor, visualiser and machining simulator, in the spirit of tools such as CIMCOEdit and NCPlot. It emphasises accurate CNC program interpretation, clear visual feedback and a modular architecture.

The long-term goal is to provide a practical environment where CNC programmers can write, edit, inspect, analyse and simulate programs before sending them to a machine. The same codebase targets desktop (Linux, Windows, macOS) and mobile (Android, iOS).

The G-code processing engine is written in Rust. A thin Tauri 2 application shell and a TypeScript user interface sit on top of it, with a clean separation between them.

> **Development status:** Early but usable. The desktop app is an editor beside a synced 3D backplot, with diagnostics and toolpath statistics, for Fanuc milling programs (Osai 10 Series as a stub). Milestones 1 (the core pipeline), 2 (render buffer and minimal desktop UI) and 3 (a dependable backplotter: real programs without false alarms, subprograms, hover, measuring, orbiting, statistics with a time estimate, million-block programs) are complete. Next is Milestone 4: code transformations. On Linux it installs as a Flatpak.

GCode EditSim is a visualisation and analysis aid. It is not a safety verification tool (see the [Safety Disclaimer](#safety-disclaimer)).

---

## Project Goals

GCode EditSim aims to bring together four capabilities:

- **G-code editing:** a dedicated editor for writing and modifying CNC programs, with source lines linked to the toolpath in both directions.
- **Toolpath visualisation (backplotting):** interactive 3D display of programmed tool movements.
- **Program analysis:** diagnostics, statistics, bounding box, tools used and estimated cycle time.
- **Program transformation:** translate, scale and rotate a selected part of a program.
- **Material removal simulation:** visual simulation of how the stock changes as the tool moves through the program.

It is built for extensibility. The user selects both the **controller dialect** (Fanuc, Haas, Siemens, Heidenhain, GRBL, LinuxCNC, ...) and the **machine type** (mill, lathe, mill-turn, multi-channel, 5-axis). Each is a separate module, so adding one does not mean rewriting the engine.

**Initial scope:** 3-axis mill with the Fanuc dialect. Everything else is designed for but not built yet.

### Non-goals

- **Not a CAM system.** It does not generate toolpaths.
- **Not a safety verification tool.** Its output must never be treated as proof that a program is safe to run.
- **Not an exact emulation of every controller option.** It targets the commonly used subset of each dialect and reports clearly what it does not support.
- **No cloud service or accounts.** Everything runs on the user's device.

### Design Principles

1. **Correctness before appearance:** accurate G-code interpretation matters more than visual sophistication.
2. **Separation of concerns:** dialect (what the text means), machine (axes, kinematics, tooling) and the intermediate representation are kept strictly apart.
3. **A pure core:** the processing engine has no UI or platform dependencies. It compiles for native targets and for WebAssembly.
4. **Data over code:** dialect code tables and machine definitions live in data files where practical.
5. **Testability:** all core functionality is tested headlessly, without the graphical application.
6. **Explicit diagnostics:** unsupported commands, invalid input and controller-dependent behaviour are reported clearly with their source location, never silently ignored.

---

## Architecture

The project is a modular Rust workspace. Its central design is a pipeline that turns G-code into a neutral intermediate representation (IR), which visualisation, analysis and simulation then consume.

```text
                     G-code source
                           |
                           v
                gcode-syntax: lexer + parser
                           |   blocks (no G-code meaning yet)
                           v
           dialect (e.g. Fanuc): codes -> commands
                           |   machine-neutral intents, in mm
                           v
       interpreter + machine model (axes, limits, tools)
                           |
                           v
            Intermediate Representation (IR)
           segments + diagnostics + source refs
                           |
         +-----------------+-----------------+
         |                 |                 |
         v                 v                 v
  render buffer      statistics,        material removal
  -> 3D backplot     cycle time         simulation
```

The full design, including the dependency rules between crates, is in [ARCHITECTURE.md](ARCHITECTURE.md).

### Components

| Path | Responsibility |
|---|---|
| `core/gcode-syntax` | Lexer and parser: G-code text to blocks, words, comments and unevaluated expressions. Knows no G-code meaning. |
| `core/ir` | The neutral intermediate representation: segments, arcs, diagnostics, source references, bounding box. |
| `core/dialect-api` | The `Dialect` trait, commands, modal state and the data-driven code-table loader. |
| `core/dialect-fanuc` | The first dialect: Fanuc milling. |
| `core/machine-api` | The machine trait: axis definitions, kinematics, limits, tools. |
| `core/machine-mill` | The first machine type: 3-axis mill. |
| `core/dialect-osai` | An Osai 10 Series dialect stub, used to check the dialect interfaces. |
| `core/machine-lathe` | A 2-axis lathe stub, used to check the machine interfaces. |
| `core/interpreter` | Combines dialect, machine and state to produce the IR. |
| `core/sim` | Material removal simulation, per machine family. |
| `core/testkit` | Golden-file test helpers. |
| `app/backend` | The app's behaviour in plain Rust: choosing machine and dialect, interpreting to a render buffer, file access. |
| `app/shell` | Tauri 2 shell: a thin command layer over `app/backend` (its own Cargo workspace). |
| `ui/` | TypeScript user interface: three.js viewport, CodeMirror editor, diagnostics panel, statistics dialog. |
| `tools/` | Development tools: the dependency-rule check, the UI constants generator, CI change detection. |
| `packaging/` | The Flatpak package and app icon. |

### Intermediate Representation

The IR is the centre of the architecture. Each controller dialect is interpreted into a common representation of machine movements and operations, rather than talking to the visualisation or simulation directly. All dialects and machine types then share the same backplotting, analysis and simulation code.

The IR preserves:

- Rapid, linear, circular and helical movements. Arcs stay exact arcs, not line approximations.
- Dwells, tool changes and synchronisation points between channels.
- Feed rate, spindle speed, active tool and work offset for every segment.
- A source reference (file, line, block) on every segment and diagnostic, linking the editor and the toolpath.
- Diagnostics with a severity and a stable code.

Positions are indexed by the machine's own axis list, so nothing downstream assumes X, Y and Z.

---

## Planned Capabilities

These are the project's intended direction. Items marked *(built)* exist today; the rest come in later milestones (see the roadmap).

### Controller Dialects

- **Fanuc first** *(built)*, starting with:
  - G0–G3 (including helical arcs)
  - planes G17–G19, inch/metric G20/G21, absolute/incremental G90/G91
  - work offsets G54–G59
  - F, S and T words
  - common M codes: stops, program end, spindle, tool change, coolant
- **Done in M3:** machine-specific M codes are noted once and skipped rather than reported as errors (a machine file can list them in `ignore_codes`); tool length compensation is noted, not simulated; G92 is ignored with a warning; M98/M99 subprogram calls within the file, with repeat counts, are followed and drawn; G95 feed per revolution.
- **On hold, left to the community:** coordinate shifts and canned cycles. They are reported and drawn without their effect, never hiding moves. Macro variables are on the wishlist.
- **Then (M6):** cutter radius compensation.
- **Not planned:** tool length (height) and wear compensation. The backplot shows the path as programmed and notes these codes.
- **Later:** further dialects (Haas, then Siemens, Heidenhain, GRBL, LinuxCNC and others), mostly as code-table overlays on a shared base.

### Machine Support

- **Machine types:** 3-axis mills first *(built)*, then lathes, mill-turn, multi-channel machines and 5-axis.
- **User-defined machines** *(built)*: described in editable text files (TOML) covering axes, travel limits, speeds and work offsets. A tool table (M6) and a graphical editor for these files will come later.

### Visualisation, Analysis and Simulation

- Interactive 3D toolpath display, with editor line and toolpath segment selection kept in sync *(built)*.
- Diagnostics panel and statistics dialog *(built)*: path lengths, bounds, feed rates, per-tool figures and a simple time estimate (no acceleration; an acceleration model comes in M6).
- Backplot tools: a tool marker with hover info on the nearest point (position, tool, feed), and measuring between two points *(built)*; orbiting around a point on the toolpath *(built)*.
- Code transformations (M4): translate, scale and rotate a selected range of lines, applied as an undoable edit.
- Tool movement playback and simulation controls.
- Travel-limit checking *(built: moves outside the travel are flagged; speed limits are checked in M6)*.
- Material removal simulation, milling first.

---

## Technology

- **Rust:** the processing engine. It is pure and platform-independent, and also compiles to WebAssembly.
- **Tauri 2:** the application shell for desktop and mobile.
- **TypeScript, three.js, CodeMirror:** the user interface, 3D viewport and editor.
- **Binary data boundary:** bulk toolpath data passes from Rust to the UI as a compact, versioned binary buffer, never as JSON.
- **TOML:** dialect code tables and machine definitions.

See `Cargo.toml` and [ARCHITECTURE.md](ARCHITECTURE.md) for details.

---

## Development Roadmap

Development is organised into milestones (details in [ARCHITECTURE.md §13](ARCHITECTURE.md#13-milestones)).

### M1 – Core Skeleton (complete, with known limitations)

The complete headless pipeline from G-code text to IR, with no UI.

- [x] Workspace, pinned toolchain and CI
- [x] `gcode-syntax` parser, with property tests and a fuzz target
- [x] `ir` types, line lookup and bounding box
- [x] `dialect-api` and the Fanuc dialect (M1 subset)
- [x] `machine-api` and `machine-mill`, including user-editable machine definitions
- [x] `interpreter` and end-to-end golden tests
- [x] Hardening after an independent review of CNC correctness (see below)

What works today, headlessly: a Fanuc milling program plus a machine definition file produce the toolpath in machine coordinates. That includes:

- work offsets applied
- reference returns (G28) and machine-coordinate moves (G53)
- exact arcs: I/J/K, R, full circles and helices
- travel-limit checks, including arc bulges
- diagnostics for everything unsupported

See [Trying it out](#trying-it-out).

**Never drawing a misleading toolpath without saying so** is the rule. The M1 review made unsupported codes hide later moves; since M3 (owner decision) moves are **never hidden**: a backplot drawn without an unsupported code's effect, with a warning, is more useful than none.

- **Codes that change later moves** (G92, G52, G10, rotation, scaling, mirroring, polar input, macro calls, unknown G codes) are reported once where they appear; their values are data, not moves, and later moves are drawn as programmed. Canned cycles draw their hole positions.
- **Unknown position.** After the machine has gone somewhere the machine file doesn't describe (G30), nothing is drawn until the position is known again.
- **Assumed defaults.** Modes a program relies on without setting them (units, G90/G91, plane, motion mode) are warned about. On a real control they depend on parameters or on the state at power-off.

**Known limitations**, tracked for later milestones:

- **Tool length compensation (G43) is not applied.** Z is shown as programmed, so Z positions and Z limit checks ignore the tool length. This is intended: height and wear compensation are not planned, and the path is shown as programmed.
- **Rapids are drawn as straight lines.** Fanuc controls often move each axis independently at its own rapid rate, a "dog-leg" path, so clearance near clamps can look better than it is (M6).
- **Speed limits aren't checked yet.** The machine file's maximum feed and rapid rates are used only by the time estimate; moves are not checked against them, and the maximum spindle speed is unused (M6).
- **Positions are machine coordinates.** Program (work) coordinates are on hold.
- **Fanuc G04 (dwell) is reported as an error** and adds no time to the estimate; Osai dwells are drawn without their time.
- **Memory:** a program of a million blocks needs about 850 MB. Fine on a desktop; to be reduced before mobile (M5).
- **Not all Fanuc behaviour is verified.** Much of it is still marked "unverified" in [docs/dialect-notes/fanuc.md](docs/dialect-notes/fanuc.md) until it is checked against a control's manual.

### M2 – Render Buffer and Minimal UI (complete)

- [x] IR events: stops, spindle, coolant, program end
- [x] Arc tessellation and the binary render buffer, with a fixture checked from both Rust and TypeScript
- [x] Stub lathe and stub Osai 10 Series dialect, to test the module boundaries
- [x] Tauri 2 desktop app, and an Android test of the binary boundary (passed on a real phone)
- [x] TypeScript UI, three.js viewport, CodeMirror editor with line and toolpath sync
- [x] Diagnostics panel, and statistics on demand in a dialog
- [x] Flatpak package for Linux
- [x] M2.8a: refresh on demand after edits, dashed rapids
- [x] M2.8b: opening and saving through the desktop's file dialogs, the machine folder, wrap-up

### M3 – A Dependable Backplotter (complete)

- [x] Real programs without false alarms: ignorable codes noted, not errors; moves never hidden
- [x] Tool marker with hover info, measuring between two points, orbiting about a point on the toolpath
- [x] Subprograms (M98/M99 within the file)
- [x] Statistics per tool with a simple time estimate; G95 feed per revolution
- [x] Programs of a million blocks open in a second or two on the owner's desktop
- [x] Along the way: resizable and maximisable panes, a colour key, the machine folder seeded with examples
- On hold, left to the community: coordinate shifts and program coordinates, canned cycles

### Later Milestones

- **M4 – Code transformations:** translate, rotate and scale a selected part of the program; remembering the machine, dialect and window layout; the editor selection shown in the 3D view; a better icon; `.nc` files opening in the app from the Linux file manager.
- **M5 – Mobile:** the same app on Android; iOS later.
- **M6 – Machine state and accuracy:** tool table, cutter radius compensation, cycle-time estimation with acceleration, the dog-leg rapid path, limits checking.
- **M7 – Material removal (mill).**
- **M8+ – More dialects and machine types:** Haas, then others; the full Osai 10 Series; lathe and mill-turn.

---

## Trying it out

On Linux the app can also be installed as a Flatpak from a single `.flatpak` file: see [packaging/flatpak/README.md](packaging/flatpak/README.md) for how to build and install it.

The desktop app shows a program in an editor beside its 3D toolpath with a diagnostics list and toolpath statistics under the view. On Linux it needs the WebKit libraries (on Ubuntu 24.04: `sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev`); then:

```bash
cd ui && npm ci && npm run build && cd ..   # the app embeds the built UI
cd app/shell && cargo run
```

The app opens with a sample program in the editor beside its toolpath. Use **Open…** (Ctrl+O) to load your own, and **Save** (Ctrl+S) or **Save As…** (Ctrl+Shift+S) to write it back; the file keeps its line endings, and the app asks before closing or opening another file with unsaved changes (shown by • after the name). Choose the machine and dialect in the toolbar (the app remembers them for next time, in `settings.json` next to the machine folder; if the remembered machine file is gone or broken, it starts with the first usable one and says so); **Machines…** shows the folder for your machine files (the app copies the example mill and router files into it on first start) and reloads the list. Click a line, or select several, to show their moves in bright yellow, or click the toolpath to jump to its line. A yellow arrow marks the tool: it sits at the point you last clicked, or where the tool is after the cursor's line, and follows the pointer over the toolpath (snapping to the ends of moves within 15 pixels), with its position (machine coordinates for now), tool and feed shown in the corner. Editing does not redraw straight away: the view gets a red border and a **Refresh (F5)** button, and clicking the toolpath or a diagnostic does nothing until you refresh, because line numbers may have moved. Under the toolpath, the **Diagnostics** panel lists every problem (click one to go to its line). Dragging with the left button from a node turns the view about that node; from anywhere else, about the centre of the view. A **Colours** key in the corner of the view says what each colour means (red is a move outside the machine's travel limits). **Measure** turns on measure mode: drag from one move end point to another to see their coordinates, the X/Y/Z differences and the distance; click **Measure** again to leave. Drag the borders between the editor, view and diagnostics to resize them. The app remembers the pane sizes, the maximised pane, whether the colour key is folded, which diagnostics are ticked, and the window's size and maximised state for next time (and its position, except under Wayland, where the desktop decides where windows go); **Reset layout** goes back to the defaults. Double-click the view or the diagnostics list to fill the window with it (the editor has a ⤢ button instead, since double-click selects a word there); double-click again or click **Exit full screen** to go back. Unticking Warnings, Errors or Notes in the diagnostics panel also hides them from the editor's margin. **Transform…** (Ctrl+T) moves, turns or scales the lines selected in the editor: choose Translate (an offset in X, Y and Z), Rotate (a plane, an angle counter-clockwise and a centre) or Scale (one factor for X and Y about a centre; Z is never scaled), in mm and program coordinates; **Pick in view** takes the centre from a node in the 3D view. Only the words that must change are rewritten, in the program's own number style, and one Undo reverts it. Whatever cannot be changed exactly (canned cycles, coordinate shifts, G53, G28/G30, an arc at the edge of the selection, …) is refused with the line named, and notes about the moves at the edges of the selection appear in the diagnostics panel; check the result in the view before running it on a machine. **Statistics…** in the toolbar works out path lengths, bounds, per-tool figures and a simple time estimate on demand and shows them in a dialog. The estimate runs each move at its feed (or the axes' rapid rates) without acceleration, so real run times are longer; a machine file can give a `tool_change_time`.

With [Rust](https://rustup.rs) installed, you can also run a program through the engine and see the result as JSON:

```bash
cargo run -p interpreter --example summary -- tests/golden/basic_mill.nc
```

That uses the example machine in `core/machine-mill/data/generic_3axis_mill.toml`. To use your own machine, copy that file, edit it (see [docs/machine-definition.md](docs/machine-definition.md)), and pass it as a second argument:

```bash
cargo run -p interpreter --example summary -- my_program.nc my_machine.toml
```

The output lists every move (end point in machine coordinates, arc centre and radius, feed, tool), every diagnostic with its line number, and the bounding box.

To run the full test suite:

```bash
cargo test --workspace
```

The user interface (`ui/`, TypeScript) needs [Node.js](https://nodejs.org) 22 or later:

```bash
cd ui && npm ci && npm run lint && npm run typecheck && npm test
```

After changing the render-buffer layout in `core/ir`, regenerate the UI's constants with `cargo run -p gen-ts`.

The app shell is a separate Cargo workspace (so the engine builds without WebKit); its tests run with `cd app/shell && cargo test`.

---

## Testing and Quality

Correctness is a priority, because the application deals with CNC machine movements. The project uses:

- **Unit tests,** and **conformance tests** for each dialect, one file per behaviour.
- **Golden-file tests:** sample programs in `tests/golden/`, each with reviewed expected output for the parser (`.syntax.txt`) and for the whole pipeline (`.ir.json`).
- **Property tests (proptest):** for example, the parser never panics on any input, and arc bounding boxes are exact.
- **Fuzzing (cargo-fuzz):** of the parser, because G-code files come from untrusted sources. CI runs a short (60 s) fuzz session on every push to `main` and on pull requests that change the parser.
- **Architecture checks:** CI fails if a crate depends on something the architecture forbids. For example, the machine model may not depend on a dialect. The rules are in `tools/depcheck/rules.toml` and are explained in [ARCHITECTURE.md §5](ARCHITECTURE.md#5-workspace-layout).
- **Checks:** formatting (`cargo fmt`), linting (`cargo clippy`, warnings as errors), a dependency audit for known vulnerabilities (`cargo audit`, `npm audit`) and continuous integration through GitHub Actions.

Unsupported or invalid G-code produces explicit diagnostics rather than being silently discarded. Every feature ships with tests.

---

## Safety Disclaimer

GCode EditSim is a visualisation and analysis aid. It is not, and is not intended to become, a CNC machine safety or verification system.

A successful simulation does not guarantee that a program is safe to execute on a physical machine.

CNC programs must always be independently checked against the target machine, controller, tooling, workholding and machining conditions.

Never rely solely on this software to prevent collisions, tool breakage or machine damage.

---

## Contributing

The project is in active development. Architectural decisions and development conventions are documented in:

- [ARCHITECTURE.md](ARCHITECTURE.md): system architecture and technical design (the source of truth).
- [CLAUDE.md](CLAUDE.md): development rules, including for AI-assisted development.
- [DEVELOPMENT_STATUS.md](DEVELOPMENT_STATUS.md): where the project stands, the decisions made so far, open issues and lessons learned. Start here when picking up work.
- [docs/plans/](docs/plans/): milestone plans: [M2](docs/plans/M2_PLAN.md) and [M3](docs/plans/M3_PLAN.md), both complete, and [M4](docs/plans/M4_PLAN.md) (next).
- [docs/dialect-notes/](docs/dialect-notes/): controller-specific behaviour and its sources.
- [docs/WISHLIST.md](docs/WISHLIST.md): ideas kept for later, not yet planned.
- [docs/machine-definition.md](docs/machine-definition.md): the machine definition file format.

Contributions, technical feedback and suggestions are welcome as the project evolves.

---

## License

GCode EditSim is licensed under the GNU Affero General Public License, version 3 only (`AGPL-3.0-only`). See [LICENSE](LICENSE).
