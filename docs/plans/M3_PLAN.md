# M3 plan: a dependable backplotter

**Status:** direction agreed with the owner (2026-10-31); details of each
step are settled in its PR. Starts after M2.8.
**Before starting:** read `CLAUDE.md`, `ARCHITECTURE.md` (especially §7.3a,
§8 and §13) and `DEVELOPMENT_STATUS.md`.

## 1. Goal (ARCHITECTURE.md §13)

> **M3: A dependable backplotter.** Real programs from CAM systems and
> hand edits draw without false alarms, with backplot tools that make the
> drawing easy to read and inspect.

The synced editor plus backplotter is already useful to many users. The
owner decided (2026-10-31) to mature it before code transformations
(M4), mobile (M5) and simulation (M6–M7).

**Done when:**

1. the owner's real Fanuc and Osai programs open with no errors except for
   things that really are unsupported, and every gap is explained;
2. the current position is easy to see (tool marker), any endpoint can be
   inspected by hovering, two points can be measured, and the view can
   orbit around a point on the toolpath;
3. canned cycles, subprograms and the common coordinate shifts are drawn;
4. statistics include a simple time estimate and per-tool figures;
5. a large program (around a million blocks) stays usable.

**Out of scope for M3:**
- changing the program text from the toolpath (M4)
- mobile (M5)
- cutter radius compensation, acceleration-based timing, the dog-leg
  rapid path (M6)
- material removal and a tool with size and shape (M7)

## 2. Owner direction (2026-10-31)

1. **Many codes can be ignored.** Machine-specific codes and codes a
   virtual machine does not need are ignored, with a short info note, not
   an error. Tool height and wear compensation (G43/G44/G49 and H/D wear
   values) are **always** ignorable: the backplot shows the path as
   programmed. Codes useful only to a few users (turret switches, some
   coordinate changes) are ignored where that still leaves a useful
   backplot.
2. **Examples from the owner's programs:** G92 on Fanuc and `(UAO,n)` on
   Osai. The agent proposed, and the owner did not object, to implement
   these properly rather than ignore them, because ignoring a shift draws
   the following moves in the wrong place (or stacks repeated parts on
   top of each other).
3. **Approximate rather than hidden.** A coordinate change that is still
   not implemented no longer hides the moves after it (the M1 motion
   blocker, ARCHITECTURE.md §7.3a). The moves are drawn, marked
   approximate (dimmed), with one warning where the code appears. To be
   designed in M3.5; ARCHITECTURE.md §7.3a changes in the same PR.
4. **Program coordinates by default.** Hover, measure and statistics show
   program (work) coordinates; machine coordinates are an option.
5. **Backplot tools in M3:** tool marker, hover info, measuring, orbit
   around a snapped point (M3.1–M3.4).
6. **Statistics and code transformations come before mobile and
   simulation.**

## 3. PR sequence

One PR per step, in this order. The viewer steps (M3.1–M3.4) come first
because they help with every test of the later steps.

### M3.0: Real-program corpus and lenient handling of ignorable codes

- **Corpus.** The owner's real programs stay private (a private
  repository, like the manuals). They are run locally with a small script
  that reports diagnostics per file; CI cannot reach them (no secrets,
  CLAUDE.md rule 8). Short excerpts the owner agrees to publish become
  golden tests in `tests/golden/`.
- **Ignorable codes.** A dialect lists codes it ignores on purpose (for
  Fanuc: G43, G44, G49 and their H words, among others found in the
  corpus). Each gives one info note per program, not one per block.
  Ignoring is never silent (CLAUDE.md "Scope discipline").
- **Machine-specific codes.** A machine file can list extra codes to
  ignore (for example a pallet or turret M code), so users need not wait
  for a release. Proposed format, to be settled in the PR:
  `[dialect] ignore = ["M50", "G150"]`.
- The `(UAO,n)` message the owner saw becomes a test.

**As built (owner decisions, 2026-10-31):**
- **Corpus:** the owner's samples are in the private repository
  `gcode-editsim-manuals` (`Fanuc_examples/`, `Osai10Series_examples/`).
  `cargo run -p corpus -- --dialect fanuc|osai [--machine FILE] FILE...`
  reports, per file, how much was drawn and every diagnostic, grouped.
  Before M3.0 every Fanuc sample drew nothing (G92 hid everything); after
  it, three draw fully and "Slotted panel" stops at its same-file
  subprogram call (M3.7). Both Osai samples draw fully.
- **G92 is ignored with a warning** (its words are data, not a move);
  proper G92 stays in M3.5. **G43/G44** give one info note per program.
  **Unknown M codes** give one info note per code. **M06 without T** is a
  warning. **`:0099`** reads as program number O0099.
- **Machine files:** `ignore_codes = ["M92", ...]` under `[machine]` (M
  codes only: a G code's axis words would be left behind and read as a
  move). The interpreter removes them before the dialect sees the block,
  so it works for every dialect. Travel limits were already optional; a
  **generic router** example ships with none and every work offset at
  zero.
- **Golden excerpts** (the owner agreed to short excerpts, comments
  removed): `tests/golden/router_fanuc_panel.nc` and
  `router_fanuc_subprogram.nc`, run on the router machine.
- The `(UAO,n,Z)` per-axis case was handled in #33; the plain `(UAO,n)`
  already worked.

### M3.1: Tool marker

- A thick vertical line with an arrowhead at the tool tip, drawn at the
  selected position (end of the selected segment, or the cursor's line in
  the editor). It is a marker only, deliberately unlike a real tool, so it
  is not mistaken for the M7 tool with size and shape.
- A constant size on screen, drawn over the toolpath.
- The existing highlight of the selected segments stays, made easier to
  see (thicker line, or a brighter colour).

### M3.2: Hover info

- When the pointer is near the toolpath, snap to the nearest **segment
  endpoint** within a few pixels, otherwise to the nearest segment.
- Show a small label with:
  - the position: X, Y, Z, labelled with the machine's axis names
  - the active tool
  - the feed rate
  - the source line
- The data is already in the render buffer (vertex positions; tool and
  feed in the segment table, §8.3), so this is UI work. Until M3.5 the
  position is in machine coordinates, and labelled so.
- The snap is computed in screen space, so picking and snapping use the
  same pixel tolerance at any zoom.

**M3.1 and M3.2 as built (brought forward into M2 at the owner's request,
2026-10-31):**
- The marker is a yellow shaft with an arrowhead whose tip is the point,
  64 pixels tall at any zoom, drawn over the toolpath; it points down Z
  (the mill's tool axis, which the views keep up).
- **Hovering moves the marker and the readout; it never moves the editor
  cursor.** A click still goes to the line. With nothing under the
  pointer, the marker rests where the tool is after the editor cursor's
  line (the end of that line's last segment, or of the last one before
  it). *Changed after review (owner, 2026-10-31):* after a click on the
  toolpath it rests at the clicked point instead, until the cursor moves
  to another line, and the readout is captioned "Machine coordinates".
- The readout (top left of the view) shows X, Y, Z in machine
  coordinates, the tool, the feed ("Rapid" for rapids), the line, and
  where the point is (end of, start of or on a move, or an event), plus
  out-of-limits and unknown-position notes.
- Snapping: the nearer end of the segment under the pointer within 10
  pixels, else the point on the path. A point at the start of a move is
  reported as the end of the move that arrived there (past any events at
  the same spot), because that move's feed brought the tool there.
- Labels are the render buffer's X, Y and Z (the mill's axes), not names
  from the machine file; that comes with program coordinates (M3.5).
- **Not done:** a stronger highlight for the selected line; the marker
  may make it unnecessary. To be decided after the owner tries it.
- **Changed after review (owner, 2026-10-31): endpoints only, and no lag.**
  Hover and clicks snap only to move end points (the end of every move,
  and a start where no move ends), within 15 pixels; farther away the
  marker goes back to its resting place, and a click does nothing. Events
  (tool changes, stops) no longer snap: they sit on move end points. The
  end points are projected to the screen once per camera change and the
  nearest is found by a simple search (`ui/src/viewport/snap.ts`),
  instead of testing every drawn line piece. The marker is now a flat SVG
  overlay (`marker.ts`), so hovering never redraws the toolpath; looking
  along Z (the Top view) it is a ring. Measured on a 19,700-move program
  in headless Chromium: about 83 ms per pointer move before (spikes to
  147 ms), no measurable cost after.

### M3.3: Measure between two points

- A measure mode: click two snapped points (endpoints, or points on a
  segment), and see the distance plus ΔX, ΔY and ΔZ. Esc cancels.
- The measurement stays drawn until cleared.

### M3.4: Orbit around the snapped point

- If a point is snapped when a rotate drag starts, the view orbits around
  that point; otherwise it orbits as now. The view must not jump when the
  orbit centre changes (move the target and keep the camera's position).

### M3.5: Program coordinates and coordinate shifts (core)

- Implement G92 properly (ignored with a warning since M3.0), relative to
  the machine file's reference position (and G92.1 if the control has it), G52 and G10
  L2/L20 on Fanuc; origins and `UAO` on Osai. Each behaviour is sourced or
  marked *(unverified)* in the dialect notes.
- The UI needs the active work offset per segment to show program
  coordinates. This likely means **render buffer layout v2** (a small
  offsets table and an index per segment), with a version bump, a new
  shared fixture and regenerated TypeScript constants (CLAUDE.md rule 4).
- Replace motion blockers by approximate drawing for what is still
  unsupported (rotation G68, scaling G51, mirroring, polar input), with a
  new segment flag. Update ARCHITECTURE.md §7.3a.

### M3.6: Canned cycles (core)

- G81–G89, G73, G76, with G98/G99 return levels and G80 cancel. Each
  cycle expands into ordinary rapid and feed segments that point back at
  the cycle's line, so line ↔ segment sync keeps working.

### M3.7: Subprograms and simple macro variables (core)

- M98/M99 within the same file (O numbers), with repeat counts.
- One line can then produce segments in several places, so the UI's line
  lookup needs a line index instead of a binary search
  (ARCHITECTURE.md §6.6).
- Simple `#` variables and arithmetic, if the corpus needs them;
  otherwise later.

### M3.8: Statistics, extended

- Per tool: feed length, rapid length, time.
- A **simple time estimate**: length over feed rate, rapids at the
  machine's rapid rate, dwells; **no acceleration** (that is M6), and
  labelled as an estimate.
- Fix the G95 feed (per revolution) stored as mm/min, which the estimate
  needs.

### M3.9: Large programs

- Measure with a generated program of about a million blocks.
- Fix what is slow; if needed, chunked buffers (§8.5), which the header
  already reserves a flag for.

### M3.10: Wrap-up

- Run the whole corpus again; README, ARCHITECTURE.md and
  `DEVELOPMENT_STATUS.md` updated; a new Flatpak for the owner to try.

## 4. Risks and how to handle them

| Risk | Mitigation |
|---|---|
| "Ignoring" a code hides a real problem | Only codes on a reviewed list, each with an info note; anything that moves the tool or shifts coordinates is implemented or drawn approximate, never ignored |
| Approximate drawing is mistaken for the real path | Dimmed and flagged; one warning at the code; never described as verified (CLAUDE.md) |
| Real programs cannot be shared publicly | Private corpus run locally; only agreed excerpts become goldens |
| Layout v2 breaks the boundary | The existing version check, shared fixture and generated constants (§8.4) |
| Subprograms break line ↔ segment order | A line index, built in the same PR as subprograms |
| Scope creep into transformations or simulation | Keep to this list; note ideas in `DEVELOPMENT_STATUS.md` |

## 5. After M3: code transformations (M4), in brief

Translate, scale and rotate a selected range of lines. The core rewrites
the program text and the editor applies it as one undoable edit. Recorded
now because it shapes M3 work:

- The core needs the exact text position of each word, so the rewrite
  keeps comments, spacing and the number style (including the Fanuc
  decimal-point rule).
- Arcs: translation and rotation are fine (I/J/K rotate with the move);
  uniform scaling scales R and I/J/K. Scaling axes by different amounts
  cannot be done with G2/G3, and is refused with a clear message rather
  than turning arcs into lines.
- Incremental (G91) moves are rotated and scaled, not translated.
- Rotation starts in the active plane, G17 first.
- A detailed `M4_PLAN.md` is written when M3 is done.
