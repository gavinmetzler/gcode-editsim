# M2 plan: render buffer and minimal UI

**Status:** proposed, awaiting the owner's answers to the questions in section 2.
**Before starting:** read `CLAUDE.md`, `ARCHITECTURE.md` (especially §8 and
§13) and `DEVELOPMENT_STATUS.md`.

## 1. Goal (ARCHITECTURE.md §13)

> **M2: Render buffer + minimal UI.** Binary boundary, three.js viewport,
> CodeMirror editor, line ↔ segment sync, stats and diagnostics panel.
> Desktop only. Add stub lathe/second dialect to stress the traits.

**Done when:** a user on Linux, Windows or macOS can:

1. open a `.nc` file and a machine file,
2. see the toolpath in 3D, coloured by move type,
3. click a line to highlight its segments, and click a segment to jump to
   its line,
4. see every diagnostic and the basic statistics.

All of this goes through the binary `RenderBuffer`, never JSON for bulk
data (CLAUDE.md rule 4).

**Out of scope for M2:**
- mobile (M3)
- cycle-time estimation (M5)
- material removal (M6)
- playback animation, unless it is cheap once the events exist
- editing features beyond basic text editing

## 2. Questions for the owner before work starts

1. **Mobile spike timing.** ARCHITECTURE.md §12 says to prototype "a
   hello-world with the buffer boundary on Android + iOS *before*
   building the UI", but §13 puts mobile in M3. **Recommendation:** do a
   small spike right after PR M2.3 (Tauri shell). If Tauri mobile can't
   pass the binary buffer, we find out before building UI on top of it.
2. **Stub lathe and second dialect timing.** **Recommendation:** early
   (PR M2.2). They exist to find trait design mistakes, and those are
   cheaper to fix before the app and UI depend on the traits. Which
   second dialect: a small **LinuxCNC** or **Haas** stub? (Haas is
   Fanuc-like, so it tests the "table overlay" idea in §7.1. LinuxCNC
   differs more, so it tests the trait boundary harder.) Recommendation:
   Haas as a table overlay, because it is the next real dialect in M7.
3. **New dependencies** (CLAUDE.md rule 7). Each PR will justify its own,
   but agree in principle to: `tauri` 2 (+ CLI), `three`, `@codemirror/*`,
   `vite`, `typescript`, `eslint`, `vitest`. Any preference, e.g. a UI
   framework (plain TS vs React or Svelte)? **Recommendation:** plain
   TypeScript with no UI framework for M2, to keep it small. Revisit if
   the UI grows.
4. **CI for the app and UI.** This needs workflow changes (CLAUDE.md rule
   9) and Linux system packages for Tauri (webkit2gtk). Please confirm a
   UI job (`npm ci`, lint, typecheck, test) and a Tauri build check on
   Linux are wanted.
5. **CLAUDE.md "Current milestone"** should change to M2. May the agent
   edit it, or will the owner?

## 3. PR sequence

Each PR is small, tested, and leaves `main` green. Steps marked
**(core)** follow every core rule (no panics, wasm-buildable, no file
access).

### M2.0: IR events (core)
*Closes review finding I4. Must land before the buffer layout is fixed.*

- Add the events the owner said matter: program stop / optional stop,
  spindle start (direction) and stop, coolant on/off, and program end.
- **Option A:** new `SegKind` values (Stop = 7, …), as zero-length
  segments like `ToolChange`.
- **Option B:** a separate event list with `SourceRef`s. A keeps the
  "everything in program order" property and the line lookup working.
  **Recommendation: A.**
- Values are fixed once the buffer exists, so add the stable-value test.
- **Interpreter:** emit them from `Command::Stop`, `Coolant`,
  `SetSpindleState` and `ProgramEnd`.
- **Goldens:** the `.ir.json` files change, so review the diffs.
- **ARCHITECTURE.md:** update §6.6.

### M2.1: Arc tessellation and RenderBuffer encoding (core, `ir`)

- `ir::tessellate(&Segment, tol) -> impl Iterator<Item = Pos>`, defined in
  §6.6 but not yet built. **Property test:** every tessellated point is
  on the arc, and the chord error is ≤ tol (required by §11). Handle full
  circles, `turns`, helices, and any plane.
- `RenderBuffer` encoder, layout **version 1 exactly as in §8.3**:
  - a 64-byte header
  - the origin as `f64x3`, with vertices stored as `f32` relative to it
    (§8.2)
  - struct-of-arrays vertices plus a per-vertex kind byte
  - a 24-byte segment table
  - a 20-byte diagnostic table
  - a UTF-8 string pool
  - every section 4-byte aligned, little-endian
- **The origin rule** needs a decision: the bounding-box centre, or the
  first point? (The bbox centre minimises `f32` error.)
- **Golden fixture:** `tests/golden/render_buffer_v1.bin` from a small
  program, plus a Rust decode test. **Any layout change bumps `version`**
  and updates the fixture (CLAUDE.md rule 4).
- **Tolerance is the caller's choice;** the UI can request it later.
- `depcheck`: no change (still `ir` only).

### M2.2: Stub lathe and stub second dialect (core)

- `core/machine-lathe` (stub): X/Z axes, `diameter_mode` on X,
  `normalize_axis_value` converts diameter to radius. Just enough to
  exercise the `Machine` trait and the machine file (`family = "lathe"`).
- `core/dialect-haas` (stub) as a Fanuc table overlay, or a minimal
  LinuxCNC (owner's choice, question 2). A handful of conformance tests.
- **Expect to revise the traits.** That is the point of this step.
  Record every change in ARCHITECTURE.md (the §7.2 and §9 "provisional"
  notes).
- `tools/depcheck/rules.toml`: add both crates. A dialect may not use
  `machine-*`, and the reverse.

### M2.3: Tauri 2 app shell (`app/`)

- Add `app/` to the workspace, and add a `depcheck` rule: `app` may use
  `interpreter`, the `*-api` crates, concrete dialects and machines, and
  `ir`. Core still never depends on `app`.
- Commands (thin, no G-code logic):
  - `interpret(program_text, machine_toml, options)` returns the
    `RenderBuffer` bytes via `tauri::ipc::Response`, never JSON
  - `load_file`, `save_file`, and listing and choosing machine files
- The app does all file access; core stays pure.
- **Off the UI thread** (§8.5): run interpretation on a worker thread.
  Chunking is deferred, but keep the header's "partial" flag reserved.
- **Tests:** Rust tests for the command layer that don't need a window.
- *(Optional, question 1)* a mobile spike branch.

### M2.4: UI scaffold and buffer decoding (`ui/`)

- Vite + TypeScript (`strict`), eslint, vitest (CLAUDE.md TS standards).
  No `any` without a comment.
- **Generated constants:** `ui/src/generated/render_buffer.ts`, produced
  from the Rust layout definition by a small tool. Probably
  `tools/gen-ts`, run in CI with a "generated file is up to date" check.
  Never edit it by hand.
- **Decoder:** typed-array views over the bytes, with no copy (§8.1).
- **Cross-boundary test (§8.4):** a vitest test decodes the same
  `render_buffer_v1.bin` fixture and checks the same values as the Rust
  test.
- **CI job** for `npm ci && npm run lint && npm run typecheck && npm test`
  (needs approval, question 4).

### M2.5: three.js viewport

- Draw segments from the vertex arrays, coloured by kind: rapid
  (dashed or muted), feed, arcs, and out-of-limits highlighted. Apply the
  `origin` on the object transform (§8.2).
- Camera: orbit, pan, zoom, fit to the bounding box, and standard views
  (top, front, side, iso).
- **Picking:** clicking a segment gives its segment index, then its
  source line.
- The UI contains **no G-code meaning** (CLAUDE.md): it only renders the
  buffer.

### M2.6: CodeMirror editor and line ↔ segment sync

- G-code syntax highlighting (lightweight; colour by token type only).
- **Line to segments:** binary search on the segment table's `src_line`,
  done in the UI with no round trip (§8.3).
  Caveat (ARCHITECTURE.md §6.6): this assumes program order, which holds
  in M1/M2 because there are no subprograms or loops.
- **Segment to line:** use `src_line` from the segment table.
- **Diagnostics in the gutter,** using the diagnostic table's line,
  severity and message.
- Re-interpret on edit, debounced and off the UI thread.

### M2.7: Diagnostics and statistics panels

- **Diagnostics list:** severity, code and message; click to jump to the
  line. Make the "assumed …" and "moves not shown" warnings easy to
  understand, since they explain gaps in the toolpath.
- **Statistics:** segment counts by kind, bounding box, tools used,
  units used, feed-move length. Cycle time is not shown (M5).
- **Wording:** never call output "verified" or "safe" (CLAUDE.md). Show
  the disclaimer.

### M2.8: Wrap-up

- Choose a machine file in the UI (from the user's files, plus the
  shipped example).
- Update the docs: README "Trying it out" for the app, ARCHITECTURE.md
  changes, and `DEVELOPMENT_STATUS.md`.
- Run a few of the **owner's real programs** end to end.

## 4. Risks and how to handle them

| Risk | Mitigation |
|---|---|
| Tauri needs system libraries (webkit2gtk) in CI and on developers' machines | Document the setup; CI installs the packages; keep core testable without Tauri |
| Binary layout drift between Rust and TS | Generated constants plus the shared `.bin` fixture decoded on both sides; a version bump on any change |
| `f32` precision far from origin | Origin-relative vertices (§8.2); a test with coordinates around ±10 000 mm |
| Large programs freeze the UI | Worker thread now; chunked buffers later (§8.5 flag reserved) |
| Trait changes ripple through the code | The stubs come early (M2.2), before the app depends on the traits |
| Scope creep into playback, editing or time estimation | Keep to the M2 list; note ideas in `DEVELOPMENT_STATUS.md` |

## 5. Carry-overs from M1 (not M2 scope unless the owner says so)

See `DEVELOPMENT_STATUS.md` sections 5–8:
- G43 (M4)
- dog-leg rapids (M5)
- speed-limit checks
- unverified Fanuc items
- the minor diagnostic-code clean-ups
- the decimal-point default
- the licence field
- nightly long fuzzing
