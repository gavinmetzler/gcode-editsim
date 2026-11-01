# M4 plan: code transformations

**Status: draft for the owner to review (2026-11-01).** Nothing is built
yet. Section 6 lists the questions that need an answer before work
starts; the rest of the plan assumes the recommended answers.
**Before starting:** read `CLAUDE.md`, `ARCHITECTURE.md` (§6, §7, §8.4a,
§8.6 and §13), `DEVELOPMENT_STATUS.md` and the notes in
`docs/plans/M3_PLAN.md` §5.

## 1. Goal (ARCHITECTURE.md §13)

> **M4: Code transformations.** Translate, scale and rotate a selected
> range of program lines; the core rewrites the text (keeping comments
> and number style) and the editor applies it as one undoable edit.

A CAM-free way to move, turn or resize part of a program: shift a
pocket 20 mm along X, turn a feature 90° about a point, or scale an
engraving to 80 %. The result is ordinary G-code text in the editor,
which the user can read, edit, undo and save.

**Done when:**

1. the user selects lines in the editor, chooses Translate, Rotate or
   Scale with its values, and the selected lines are rewritten in place,
   as one edit that a single Undo reverts;
2. only the words that must change are rewritten: comments, spacing,
   N numbers, word order, other words and unselected lines are untouched,
   byte for byte;
3. numbers keep the program's style, including the Fanuc decimal-point
   rule (`X10` without a point is 0.010 mm), in mm and in inches;
4. the toolpath of the rewritten lines is the original path transformed,
   checked by tests that run both through the interpreter;
5. whatever cannot be transformed exactly is refused with a clear
   message, or transformed with a warning, never changed silently.

**Out of scope for M4:**
- mirroring (wishlist, or M4 if the owner wants it: question 2)
- non-uniform scaling of arcs (refused, as agreed)
- transforming rotary axes, canned-cycle depths in incremental mode, and
  the codes M3 left on hold (coordinate shifts, macros): refused with a
  message (question 4)
- the Osai dialect and the lathe (M8); the design keeps the dialect
  boundary, so they can be added
- mobile (M5)

## 2. How it works, in brief

1. **The user selects lines** in the editor (whole lines; a partial
   selection grows to whole lines) and opens **Transform…** (toolbar,
   or a shortcut).
2. **A dialog** asks for the transformation:
   - Translate: dX, dY, dZ.
   - Rotate: angle (degrees, counter-clockwise looking down the plane's
     third axis) and centre, in the active plane (G17 XY first; G18 and
     G19 as the program selects them).
   - Scale: factor and centre (uniform; question 3 covers per-axis
     factors for programs without arcs).
   The centre can be typed, or picked by clicking a node in the 3D view
   (the M3.3 snapping), which fills in its program coordinates.
3. **The core rewrites the lines.** The UI sends the whole program text
   (raw bytes, like `interpret`), the selected line range and the
   transformation (small JSON). The core:
   - runs the program as usual to learn, for every block in the range,
     the modal state (G90/G91, plane, units, work offset, decimal-point
     rule) and the full programmed position before and after it, in
     **program coordinates** (the coordinates the text is written in);
   - works out each block's new words: target X/Y/Z, arc centre I/J/K
     or R, and any axis word that must be added (rotating `G1 X10.`
     about Z changes Y too, so a Y word is added);
   - formats each new number in the program's style and returns the
     edits: for each changed word its line, byte span and new text
     (spans come from the parser, which already records them), plus
     warnings.
4. **The editor applies the edits** as one CodeMirror transaction (one
   Undo), and the program runs again at once, like loading a file, so the
   view shows the result. Warnings appear in the diagnostics panel.

The core does all the G-code work; the UI only collects values, picks a
centre and applies the returned edits (CLAUDE.md: no G-code meaning in
the UI). The edits come back as a compact binary list, not JSON (rule 4:
a selection can be most of a large program).

## 3. Rules for each transformation

Positions are in program coordinates throughout: a transformation moves
the part, not the machine. Machine-coordinate moves (G53) and reference
returns (G28/G30) are not transformed (question 4).

**Absolute moves (G90).** The block's full target position is
transformed, and every axis word whose value changes is rewritten. An
axis that was modal (not written) but now changes gets a word added
after the last axis word of the block.

**Incremental moves (G91).** Each move's delta is rotated or scaled (a
translation does not change deltas). Translating an incremental section
is the open question 1.

**Arcs (G2/G3).**
- I/J/K are offsets from the start (Fanuc default), so they are rotated
  and scaled with the move and unchanged by a translation.
- R is unchanged by translation and rotation and scaled by a uniform
  scale (a negative scale factor is refused: it would mirror).
- An arc whose plane is not the rotation plane is refused (for example,
  a G18 arc rotated about Z), because it would leave its plane.
- Helical arcs: the axis outside the plane is translated and scaled like
  a line; rotation is about that axis, so it is unchanged.

**The first and last block of the selection.** The first move starts
from the (untransformed) point before the selection, and the first move
after the selection starts from the transformed end point. Both
connecting moves change their path, as they must; a note in the
diagnostics names them so the user can check them in the view.

**Feeds and speeds** are not changed. A scaled path at the same feed
takes a different time; the statistics show it.

**Units.** Values are converted to the program's units (inch or mm) at
the time of each block, so a G20 section stays in inches.

**Number style** (the hard part of "keeping the program's style"):
- A value written with a decimal point keeps one, with as many decimals
  as it had or as the result needs, up to the least input increment
  (3 decimals in mm, 4 in inches), and no more.
- A value written without a decimal point keeps none, in the dialect's
  convention: under the Fanuc default (DPI = 0) `X10` means 10 least
  increments (0.010 mm), so the new value is written in least
  increments too; with the calculator option it is whole mm.
- Values are rounded to the least input increment; the rounding error is
  at most half an increment (0.0005 mm), and tests check the transformed
  path against the exact one within that.
- A value written with a leading zero, sign or `+` keeps that style
  where it still applies.
These rules live in the dialect (a new `Dialect` method to format a
value as that controller reads it), so other dialects bring their own.

**Subprograms.** Lines inside a subprogram are transformed once in the
text, which changes every call of it; a warning says so when the
subprogram runs more than once. A selection containing an M98 call does
not transform the called subprogram (it is elsewhere in the file); a
warning says so.

**Refused, with a message** (question 4): a selection containing G53,
G28/G30 intermediate points, canned cycles, G92/G52/G10, macro
variables or expressions, or a block the dialect reported as an error.
The message names the first such line. Nothing is changed.

## 4. PR sequence

One PR per step, in this order. Each starts with failing tests.

### M4.0: This plan
Agreed with the owner; ARCHITECTURE.md §13 and CLAUDE.md point to it.

### M4.1: Exact rewriting (core)
- A new core crate `transform` (depends on `gcode-syntax`, `dialect-api`,
  `ir`, `machine-api` and `interpreter`, not on concrete dialects), and
  its place in `tools/depcheck/rules.toml` and ARCHITECTURE.md §5.
- Applying word edits to the text by byte span, leaving every other byte
  as it was.
- `Dialect::format_value` for Fanuc (decimal point, least increment,
  units), with property tests: formatting a value and reading it back
  gives the same value within half an increment, and rewriting a word
  with its own value leaves the text unchanged.
- The identity transformation leaves any program unchanged, byte for
  byte (property test over generated and golden programs).

### M4.2: Positions in program coordinates (core)
- For each block: modal state and the full programmed position before
  and after, in program coordinates and program units, from the
  interpreter (a per-block trace alongside the segments, only when
  asked, so normal runs pay nothing).
- Tests: absolute and incremental, units changes, work offset changes,
  subprogram calls.

### M4.3: Translate (core)
- G90 moves, arcs, helices, added words; the rules in §3; the refusals.
- The check for all three transformations: run the original and the
  rewritten program; the rewritten selection's segments equal the
  original's transformed, within the rounding tolerance (property test).

### M4.4: Rotate (core)
- In G17, G18 and G19; added axis words; arcs and helices; incremental
  sections.

### M4.5: Scale (core)
- Uniform, about a centre; R and I/J/K; refusal of a negative or zero
  factor; per-axis factors only if question 3 says yes.

### M4.6: The app and UI
- Shell command `transform` (raw program in, binary edit list out,
  parameters in small JSON) and its IPC tests.
- UI: Transform… dialog, line-range selection, centre picked from the
  view, applying the edits as one transaction, running again, showing
  warnings. Unit tests for the pure parts; a browser check.

### M4.7: Wrap-up
- The corpus and goldens: transform each private program with a few
  transformations and check the path (locally); a short public golden
  excerpt if the owner agrees.
- README, ARCHITECTURE.md, DEVELOPMENT_STATUS.md; a new Flatpak to try.

## 5. Risks and how to handle them

| Risk | Mitigation |
|---|---|
| A rewritten program is subtly wrong and machined | Tests compare the interpreted path before and after for every transformation; refuse what cannot be done exactly; the app never calls a rewrite verified (CLAUDE.md). Undo is one step. |
| Number formatting changes meaning (Fanuc `X10` vs `X10.`) | Formatting lives in the dialect; property tests read every written value back through the same dialect |
| Rounding drift over many lines | Every value is computed from the original exact position, not from the previous rewritten one |
| Edits shift byte positions | Edits are applied from the end of the line backwards, and only within the selected lines |
| Large selections | Edits cross the boundary as a binary list; the trace is only built for the selected range |
| Scope creep (mirror, per-axis scale, canned cycles) | Section 6; anything not agreed goes to `docs/WISHLIST.md` |

## 6. Questions for the owner

1. **Translating incremental (G91) moves.** Deltas do not change, so to
   move an incremental section the first move into it must gain the
   offset and the first move after it must lose it, which changes a line
   outside the selection. Recommended: do that, and say so in the
   message; the alternative is to refuse translation of selections that
   start in G91.
2. **Mirroring** (flipping X or Y, which also swaps G2 and G3). Not in
   the agreed M4 list. Recommended: wishlist, unless you use it.
3. **Per-axis scale factors** for selections without arcs (stretching
   an engraving in X only). Recommended: allow it when the selection has
   no arcs, refuse with a clear message when it does.
4. **Unsupported codes in the selection** (G53, G28/G30, canned cycles,
   G92/G52/G10, macros). Recommended: refuse the whole transformation and
   name the line, rather than transform the rest and leave those as
   they are.
5. **Preview.** Show the transformed path as a ghost in the 3D view
   while the dialog is open, before applying. Recommended: not in M4
   (undo is one step, and the program runs again at once); wishlist.
