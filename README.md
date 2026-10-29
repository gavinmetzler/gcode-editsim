# GCode EditSim

**A modern G-code editor and CNC milling simulator.**

GCode EditSim is an open-source project to develop a dedicated G-code editor, visualiser and milling simulator, with an emphasis on accurate CNC program interpretation, clear visual feedback and a modular architecture.

The long-term goal is to provide a practical environment where CNC programmers can write, edit, inspect and simulate milling programs before sending them to a machine.

The project is being developed in Rust, with a clean separation between the G-code processing engine, application logic and user interface.

> **Development status:** Early development — Milestone 1 (Core Foundation).

The application is not yet suitable for production CNC verification or machine operation.

---

## Project Goals

GCode EditSim aims to bring together three essential capabilities:

- **G-code editing** – A dedicated editor for writing and modifying CNC programs.
- **Toolpath visualisation** – Interactive 3D visualisation of programmed tool movements.
- **Material removal simulation** – Visual simulation of the machining process, showing how the stock changes as the tool moves through the program.

The project is being designed with extensibility in mind, allowing support for different CNC controller dialects and machine configurations over time.

### Design Principles

The project follows several core principles:

1. **Correctness before appearance** – Accurate G-code interpretation is more important than visual sophistication.
2. **Separation of concerns** – Parsing, interpretation, geometry, simulation and rendering should remain independent.
3. **Testability** – Core functionality should be independently testable without requiring the graphical application.
4. **Extensibility** – Controller dialects and machine configurations should be supported without rewriting the underlying engine.
5. **Explicit diagnostics** – Unsupported commands, invalid input and interpretation problems should be reported clearly rather than silently ignored.

---

## Architecture

The project uses a modular Rust workspace.

Its central design is a processing pipeline that transforms G-code into a neutral intermediate representation (IR), which can then be consumed by visualisation and simulation systems.

```text
                G-code Source
                     |
                     v
                Syntax Parser
                     |
                     v
             Dialect Interpreter
                     |
                     v
              Machine Interpreter
                     |
                     v
          Intermediate Representation
                     |
             +-------+-------+
             |               |
             v               v
         Backplotter     Simulator
             |               |
             v               v
         3D Toolpath    Material Removal
```

### Main Components

| Component | Responsibility |
|---|---|
| `gcode-syntax` | Parsing G-code source into structured syntax |
| `ir` | Defining the neutral intermediate representation |
| `core` | Core CNC interpretation and processing |
| `app` | Application-level orchestration |
| `ui` | User interface and visual presentation |

The exact implementation boundaries are defined in [ARCHITECTURE.md](ARCHITECTURE.md).

### Intermediate Representation

The Intermediate Representation is a central part of the architecture.

Rather than allowing each controller dialect to communicate directly with the visualisation or simulation systems, G-code is interpreted into a common representation of machine movements and operations.

This allows different controllers to share the same downstream geometry, backplotting and simulation infrastructure.

The IR is intended to preserve important information such as:

- Linear movements.
- Circular movements and arcs.
- Helical interpolation.
- Rapid positioning.
- Feed movements.
- Tool changes.
- Source-code references.
- Diagnostic information.

---

## Planned Capabilities

The following capabilities represent the project's intended direction. They are not necessarily implemented yet.

### G-code Support

- G0 and G1 linear movements.
- G2 and G3 circular interpolation.
- Absolute and incremental positioning.
- XY, XZ and YZ arc planes.
- Work coordinate systems.
- Tool length compensation.
- Cutter radius compensation.
- Canned drilling cycles.
- Tool changes.
- Controller-specific dialect support.

### Machine Support

The architecture is intended to accommodate different machine configurations, including:

- Three-axis milling machines.
- Four-axis milling machines.
- Five-axis milling machines.
- Additional machine types as the project evolves.

Initial development is focused on three-axis milling.

### Visualisation and Simulation

Planned visual capabilities include:

- Interactive 3D toolpath display.
- Tool and stock visualisation.
- Tool movement playback.
- Simulation controls.
- Material removal visualisation.
- Machine movement inspection.
- Detection of potential machining problems.

---

## Technology

The project uses Rust as its primary implementation language.

Key technologies and architectural choices include:

- **Rust** – Core application and processing engine.
- **Cargo** – Workspace and dependency management.
- **TypeScript** – UI-side integration.
- **Rust-to-TypeScript interface** – Communication between the processing engine and visualisation layer.
- **Golden-file testing** – Regression testing of parsing and interpretation behaviour.

See `Cargo.toml` and `ARCHITECTURE.md` for implementation details.

---

## Development Roadmap

Development is organised into milestones.

### M1 – Core Foundation

Establish the underlying Rust workspace and core G-code processing architecture.

Current development is focused on:

- Workspace and crate structure.
- G-code syntax parsing.
- Intermediate representation.
- Core data types.
- Diagnostics.
- Unit and regression testing.

### M2 – Core Interpretation

Develop the functionality required to interpret G-code into meaningful machine movements and operations.

### M3 – Application and UI Foundation

Develop the application interface and establish communication between the Rust engine and the UI.

### Future Milestones

Further development will focus on:

- Interactive toolpath visualisation.
- Machine configuration.
- Material removal simulation.
- Editing and playback functionality.
- Expanded controller support.
- Additional machine types.

The roadmap will evolve as the underlying engine develops.

---

## Testing and Quality

Correctness is a priority, particularly because the application deals with CNC machine movements.

The project uses:

- Rust unit tests.
- Golden-file regression tests.
- Automated formatting checks.
- Clippy linting.
- Continuous integration through GitHub Actions.

Unsupported or invalid G-code should generate explicit diagnostics rather than being silently discarded.

New functionality should include appropriate regression tests.

---

## Safety Disclaimer

GCode EditSim is a development project and is not currently certified or validated as a CNC machine safety or verification system.

A successful simulation does not guarantee that a program is safe to execute on a physical machine.

CNC programs must always be independently checked against the target machine, controller, tooling, workholding and machining conditions.

Never rely solely on this software to prevent collisions, tool breakage or machine damage.

---

## Contributing

The project is currently under active development.

Architectural decisions and development conventions are documented in:

- [ARCHITECTURE.md](ARCHITECTURE.md) – System architecture and technical design.
- [CLAUDE.md](CLAUDE.md) – Development guidelines and instructions for AI-assisted development.

Contributions, technical feedback and suggestions are welcome as the project evolves.

---

## License

License information will be added as the project progresses.
