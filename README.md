# GCode EditSim

**A modern G-code editor, backplotter and CNC simulator for desktop and mobile.**

GCode EditSim is an open-source project to develop a dedicated G-code editor, visualiser and machining simulator, in the spirit of tools such as CIMCOEdit and NCPlot. It emphasises accurate CNC program interpretation, clear visual feedback and a modular architecture.

The long-term goal is to provide a practical environment where CNC programmers can write, edit, inspect, analyse and simulate programs before sending them to a machine. The same codebase targets desktop (Linux, Windows, macOS) and mobile (Android, iOS).

The G-code processing engine is written in Rust. A thin Tauri 2 application shell and a TypeScript user interface sit on top of it, with a clean separation between them.

> **Development status:** Early development. Milestone 1 (the headless core pipeline) is complete; there is no UI yet. Next is Milestone 2: render buffer and a minimal desktop UI.

GCode EditSim is a visualisation and analysis aid. It is not a safety verification tool (see the [Safety Disclaimer](#safety-disclaimer)).

---

## Project Goals

GCode EditSim aims to bring together four capabilities:

- **G-code editing:** a dedicated editor for writing and modifying CNC programs, with source lines linked to the toolpath in both directions.
- **Toolpath visualisation (backplotting):** interactive 3D display of programmed tool movements.
- **Program analysis:** diagnostics, statistics, bounding box, tools used and estimated cycle time.
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
| `app/` | Tauri 2 shell: a thin command layer over the core. |
| `ui/` | TypeScript user interface: three.js viewport, CodeMirror editor. |

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

These are the project's intended direction. Most are not implemented yet; see the roadmap.

### Controller Dialects

- **Fanuc first**, starting with:
  - G0–G3 (including helical arcs)
  - planes G17–G19, inch/metric G20/G21, absolute/incremental G90/G91
  - work offsets G54–G59
  - F, S and T words
  - common M codes: stops, program end, spindle, tool change, coolant
- **Then:** tool length compensation, canned drilling cycles, subprograms, macro variables and cutter radius compensation.
- **Later:** further dialects (Haas, then Siemens, Heidenhain, GRBL, LinuxCNC and others), mostly as code-table overlays on a shared base.

### Machine Support

- **Machine types:** 3-axis mills first, then lathes, mill-turn, multi-channel machines and 5-axis.
- **User-defined machines:** described in editable text files (TOML) covering axes, travel limits, maximum speeds, tool table format and default work offsets. A graphical editor for these files will come later.

### Visualisation, Analysis and Simulation

- Interactive 3D toolpath display, with editor line and toolpath segment selection kept in sync.
- Diagnostics and statistics panels: bounding box, tools used, estimated cycle time.
- Tool movement playback and simulation controls.
- Travel-limit checking.
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

**Never drawing a misleading toolpath** is the rule the review hardened.

- **Hidden moves.** Unsupported codes that change where *later* moves go (G92, G52, G68, scaling, mirroring, polar input, subprograms, canned cycles, unknown G codes) hide the moves they affect. This is explained once, where the code appears. Nothing is drawn in the wrong place.
- **Unknown position.** After the machine has gone somewhere that isn't simulated, nothing is drawn until the position is known again.
- **Assumed defaults.** Modes a program relies on without setting them (units, G90/G91, plane, motion mode) are warned about. On a real control they depend on parameters or on the state at power-off.

**Known limitations**, tracked for later milestones:

- **Tool length compensation (G43) is not applied.** Z is shown as programmed, so Z positions and Z limit checks ignore the tool length (M4).
- **Rapids are drawn as straight lines.** Fanuc controls often move each axis independently at its own rapid rate, a "dog-leg" path, so clearance near clamps can look better than it is (M5).
- **Speed limits aren't checked yet.** The machine file's maximum feed and spindle speed are loaded but not used.
- **Not all Fanuc behaviour is verified.** Much of it is still marked "unverified" in [docs/dialect-notes/fanuc.md](docs/dialect-notes/fanuc.md) until it is checked against a control's manual.

### Later Milestones

- **M2 – Render buffer and minimal UI:** binary Rust/UI boundary, 3D viewport, editor, line and toolpath sync, statistics and diagnostics. Desktop only. Stub lathe and second dialect to test the module boundaries.
- **M3 – Mobile spike:** the same app on Android and iOS.
- **M4 – Machine state:** tool changes and tool table, tool length compensation, canned cycles, subprograms, simple macro variables.
- **M5 – Accuracy:** cutter radius compensation, cycle-time estimation with acceleration, limits checking.
- **M6 – Material removal (mill).**
- **M7+ – More dialects and machine types:** Haas, then others; lathe and mill-turn.

---

## Trying it out

The desktop app shows a program's toolpath in 3D (the editor and panels come next). On Linux it needs the WebKit libraries (on Ubuntu 24.04: `sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev`); then:

```bash
cd ui && npm ci && npm run build && cd ..   # the app embeds the built UI
cd app/shell && cargo run
```

The app opens with a sample program. Use **Open…** to load your own, choose the machine and dialect in the toolbar, and click the toolpath to see which line made it.

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
- **Fuzzing (cargo-fuzz):** of the parser, because G-code files come from untrusted sources. CI runs a short (60 s) fuzz session on every change.
- **Architecture checks:** CI fails if a crate depends on something the architecture forbids. For example, the machine model may not depend on a dialect. The rules are in `tools/depcheck/rules.toml` and are explained in [ARCHITECTURE.md §5](ARCHITECTURE.md#5-workspace-layout).
- **Checks:** formatting (`cargo fmt`), linting (`cargo clippy`, warnings as errors) and continuous integration through GitHub Actions.

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
- [docs/plans/M2_PLAN.md](docs/plans/M2_PLAN.md): the plan for the next milestone.
- [docs/dialect-notes/](docs/dialect-notes/): controller-specific behaviour and its sources.
- [docs/machine-definition.md](docs/machine-definition.md): the machine definition file format.

Contributions, technical feedback and suggestions are welcome as the project evolves.

---

## License

GCode EditSim is licensed under the GNU Affero General Public License, version 3 only (`AGPL-3.0-only`). See [LICENSE](LICENSE).
