# M2 plan: render buffer and minimal UI

**Status:** agreed. The owner answered the section 2 questions on 2026-10-30
(recorded there); work proceeds one PR at a time from M2.0. Milestones
after M2 were renumbered on 2026-10-31 (ARCHITECTURE.md §13); references
to them below use the new numbers.
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
- mobile (now M5)
- cycle-time estimation (now M3.8, simple; M6 with acceleration)
- material removal (now M7)
- playback animation, unless it is cheap once the events exist
- editing features beyond basic text editing

## 2. Decisions (owner's answers, 2026-10-30)

1. **Mobile spike: yes.** A small Android/iOS test of the binary boundary
   right after the Tauri shell (M2.3), before building UI on top of it.
2. **Stub lathe: yes, early (M2.2).** **Second dialect: Osai 10 Series**
   (not Haas or LinuxCNC). Osai differs from Fanuc more than Haas does, so
   it tests the dialect trait harder; it is a stub, not a full dialect.
   Every Osai behaviour must be sourced from the Osai 10 Series
   programming manual or marked *(unverified)* in a new
   `docs/dialect-notes/osai.md`; nothing is assumed from Fanuc.
3. **Dependencies: agreed in principle,** left to the agent's judgement.
   Each PR still justifies its own (CLAUDE.md rule 7). **Plain TypeScript,
   no UI framework.**
4. **CI: yes, on this occasion only.** The agent may add the UI job and
   the Linux Tauri build check in M2. Any later workflow change needs the
   owner's permission again (CLAUDE.md rule 9).
5. **CLAUDE.md "Current milestone":** the agent updates it (done with this
   plan).

M2.3 decisions (owner, before M2.3):

6. **The Tauri shell is its own small Cargo workspace** (`app/shell`), so
   the core and its CI need no WebKit. Its behaviour lives in
   `app/backend` (`app-backend`), a normal workspace crate.
7. **App identity:** name "GCode EditSim", bundle identifier
   `com.ncsoftbyai.gcodeeditsim`.
8. **Mobile spike: Android only.** iOS waits until much later (it needs a
   Mac).
9. **The Osai stub is offered in the UI's dialect list** (marked as an
   early stub).

Smaller choices left to the agent's judgement:
- **M2.0:** Option A, new zero-length `SegKind` values, to keep program
  order and the line lookup.
- **M2.1:** the render-buffer origin is the bounding-box centre (least
  `f32` error).

## 3. PR sequence

Each PR is small, tested, and leaves `main` green. Steps marked
**(core)** follow every core rule (no panics, wasm-buildable, no file
access).

**Status (2026-10-31):** M2.0–M2.7 merged, plus work added along the way:
the Android spike (M2.3), a Flatpak package for Linux, and CI jobs that
run only when their files change. M2.8a and M2.8b complete M2.

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
- `core/dialect-osai` (stub): Osai 10 Series, a handful of codes with
  conformance tests and sourced notes in `docs/dialect-notes/osai.md`.
  Its own code table, not a Fanuc overlay, so the shared parts must be
  in `dialect-api`.
- **Expect to revise the traits.** That is the point of this step.
  Record every change in ARCHITECTURE.md (the §7.2 and §9 "provisional"
  notes).
- `tools/depcheck/rules.toml`: add both crates. A dialect may not use
  `machine-*`, and the reverse.

### M2.3: Tauri 2 app shell (`app/`)

Two PRs: **M2.3a** `app/backend` (all behaviour, plain Rust, tested in the
normal workspace) and **M2.3b** `app/shell` (Tauri, its own workspace, a
Linux CI build job).

- `app-backend` gets a `depcheck` rule: it may use `interpreter`, the
  `*-api` crates, concrete dialects and machines, and `ir`. Core still
  never depends on it. It picks the machine type from `family`.
- Commands (thin, no G-code logic):
  - `interpret(program_text, machine_toml, options)` returns the
    `RenderBuffer` bytes via `tauri::ipc::Response`, never JSON
  - `load_file`, `save_file`, and listing and choosing machine files
- The app does all file access; core stays pure.
- **Off the UI thread** (§8.5): run interpretation on a worker thread.
  Chunking is deferred, but keep the header's "partial" flag reserved.
- **Tests:** Rust tests for the command layer that don't need a window.
- **Then the mobile spike** (decisions 1 and 8): the same command returning
  the buffer on Android. A throw-away branch unless it is cheap to keep.
  **Done:** ALL PASSED on the owner's arm64 phone (2026-10-30). The
  program went in as a raw body, the render buffer came back as raw bytes
  matching the desktop build, a 20 000-block program ran, and save/load
  round-tripped all 256 byte values. Kept on branch `claude/android-spike`
  with `app/shell/SPIKE.md` (tool versions, the mobile changes, and how to
  rebuild the APK) for mobile (now M5).

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
  (approved for M2, decision 4).
- **As built:** TypeScript 6.0 rather than 7.0, because typescript-eslint
  does not support 7 yet. The generated-file check is a `gen-ts` test, so it
  runs in the existing Rust job. The app shell keeps its placeholder page
  until M2.5, when there is something to show; switching it to serve
  `ui/dist` then also means the shell's CI job builds the UI first.

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
- **As built (M2.5a):** `ui/src/viewport/`.
  - `toolpath.ts` (pure, tested) builds the index lists (each segment's own
    vertices, never joined across segments), per-vertex colours by kind
    and flags, event markers and a vertex-to-segment map.
  - `viewport.ts` draws feeds solid, rapids meant to be dashed (dashes
    scaled to the toolpath) and events as markers, with orbit, pan and
    zoom. *Found later:* the dashes never showed, because three.js
    computes line distances only for non-indexed geometry; fixed in M2.8.
  - **Camera:** an **orthographic** camera, as backplots usually are. A
    perspective camera made heights shift positions in the top view.
  - **Standard views:** Top, Front, Right and Iso, all with Z up, plus Fit.
  - **Picking:** shows the segment's source line.
  - **Axes marker:** at machine zero.
  - **Sample program:** the shipped `basic_mill.nc` loads at start.
  - **Checking:** checked visually in the real app under Xvfb with
    screenshots, as well as by unit tests of the pure parts.
- **M2.5b** (workflow change approved by the owner): the app shell serves
  `ui/dist` instead of the placeholder; the shell's CI job builds the UI
  first.

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
- **As built:** CodeMirror 6 (separate packages, no bundled setup) in
  `ui/src/editor/`.
  - `gcode-language.ts`: highlighting by token type only.
  - `lines.ts` (pure, tested):
    - the line → segment binary search, matching `ir::segments_for_line`
    - diagnostics → gutter marks
    - `latestOnly`, which re-runs 300 ms after the last edit and drops a
      result that finishes after a newer run started
  - The editor's text (lines joined by `\n`) is what is interpreted, so
    editor and parser line numbers always agree, whatever the file's line
    endings.
  - Edits keep the camera; a new file, machine or dialect refits it.
  - The content security policy now allows inline *styles*, because
    CodeMirror injects them. Scripts are unchanged.
  - Checked in the real app under Xvfb:
    - highlighting and gutter marks
    - clicking a line highlights its segments
    - clicking the toolpath jumps to its line
    - typing re-runs the program, keeping the view
  - **Not yet:** saving files (needs a save dialog with a real path; M2.8).

### M2.7: Diagnostics and statistics panels

- **Diagnostics list:** severity, code and message; click to jump to the
  line. Make the "assumed …" and "moves not shown" warnings easy to
  understand, since they explain gaps in the toolpath.
- **Statistics:** segment counts by kind, bounding box, tools used,
  units used, feed-move length. Cycle time is not shown (a simple estimate comes in M3.8).
- **Wording:** never call output "verified" or "safe" (CLAUDE.md). Show
  the disclaimer.
- **Done (as built):** a tabbed panel under the viewport
  (`ui/src/panels/`). Diagnostics are listed whole-program first, then by
  line, with severity filters; clicking a row moves the editor cursor,
  which highlights the line's toolpath. Statistics: feed and rapid
  length, feed-rate range, bounds of feed moves and of all motion
  (machine coordinates), feed and rapid length per tool, counts of
  out-of-limits and unknown-position segments, and segment counts by
  kind. All worked out in the UI from the render buffer, so the layout
  did not change. **Not shown:** the units the program used, which the
  render buffer does not carry (everything is in mm); it can come with
  layout v2 (M3.5).
- **Changed after review (owner, 2026-10-31):** statistics moved out of
  the panel into a dialog opened from the toolbar (**Statistics…**). They
  are worked out only when asked for, since a very large program takes a
  while, and every section shows at once without scrolling; OK or Esc
  closes it. The panel under the viewport is diagnostics only.

### M2.8: Wrap-up

Two PRs (owner, 2026-10-31).

**M2.8a: refresh on demand, dashed rapids, wishlist** *(done)*
- **Refresh on demand** (owner's request, not in the original plan): on a
  large program, re-running after every edit makes typing feel slow, and
  half-typed text is often broken. Edits now mark the view out of date: a
  red border and a **Refresh (F5)** button on the view, the diagnostics
  and gutter marks dimmed, and line sync off (clicking the toolpath or a
  diagnostic does nothing) until the refresh, because line numbers may
  have moved. Loading a file or changing the machine or dialect still
  runs at once. `ui/src/freshness.ts` (pure, tested) tracks it.
- **Dashed rapids:** fixed (and later, after M3.4, made solid again at the
  owner's request: dashes looked cluttered on real programs; rapids are
  told apart by colour). Rapids were drawn from a non-indexed copy of
  their vertices, so three.js can measure dash distances.
- **`docs/WISHLIST.md`:** started, with a settings dialog whose first
  option is automatic updates versus refresh on demand.

**M2.8b: saving, machine folder, wrap-up** *(done)*
- **Saving:** Save and Save As through a native file dialog (the open
  file's real path is needed to save over it).
- **Machine files:** the toolbar already lists the shipped example and
  the files in the user's machine folder; make that folder easy to find
  and the list easy to refresh.
- **Flatpak:** a new build for the owner to try.
- Update the docs: README "Trying it out" for the app, ARCHITECTURE.md
  changes, and `DEVELOPMENT_STATUS.md`.
- Run a few of the **owner's real programs** end to end.
- **As built:**
  - **Open…, Save, Save As…** (Ctrl+O, Ctrl+S, Ctrl+Shift+S) use the
    native dialogs (`tauri-plugin-dialog` 2.8.4; new dependency, see the
    PR). The UI gets a real path; the app's `load_file` and `save_file`
    do the reading and writing. The window may use only the open, save
    and message dialogs and the close-request event.
  - Saving keeps the file's line endings (`ui/src/files.ts`). A file that
    was not UTF-8 asks before saving, since its unreadable bytes would
    change. A • after the name marks unsaved changes; opening another
    file, the sample, or closing the window asks first.
  - **Machines…** shows the user's machine folder (inside the Flatpak,
    the sandbox's config folder) and reloads the list, keeping the chosen
    machine if it is still there.
  - The toolbar wraps on narrow windows, keeping the view buttons
    together on the right.
  - Checked inside the Flatpak with the desktop portals: Open and Save As
    through the host's file chooser, Save over the file, CRLF kept
    (`packaging/flatpak/README.md`, "Tested").
  - **Not done:** the owner's real programs end to end (they stay
    private; that is M3.0's corpus).

## 4. Risks and how to handle them

| Risk | Mitigation |
|---|---|
| Tauri needs system libraries (webkit2gtk) in CI and on developers' machines | Document the setup; CI installs the packages; keep core testable without Tauri |
| Binary layout drift between Rust and TS | Generated constants plus the shared `.bin` fixture decoded on both sides; a version bump on any change |
| `f32` precision far from origin | Origin-relative vertices (§8.2); a test with coordinates around ±10 000 mm |
| Large programs freeze the UI | Worker thread now; chunked buffers later (§8.5 flag reserved) |
| Trait changes ripple through the code | The stubs come early (M2.2), before the app depends on the traits |
| No Osai 10 Series manual to hand; behaviour guessed from Fanuc | Resolved: the owner provided the manual (private repository); unsourced behaviour is still marked *(unverified)*; the stub stays small |
| Scope creep into playback, editing or time estimation | Keep to the M2 list; note ideas in `DEVELOPMENT_STATUS.md` |

## 5. Carry-overs from M1 (not M2 scope unless the owner says so)

See `DEVELOPMENT_STATUS.md` sections 5–8:
- G43: not planned; ignored with an info note (M3.0, owner decision 2026-10-31)
- dog-leg rapids (M6)
- speed-limit checks
- unverified Fanuc items
- the minor diagnostic-code clean-ups
