# M4 plan: code transformations

**Status: agreed with the owner (2026-11-01).** The owner's answers to
the open questions are in section 6, and the owner added five items:
remembering the machine and dialect and the window layout (moved from
the wishlist), the editor selection shown in the 3D view, a better icon,
and `.nc` files opening from the file manager. Details of each step are settled in its
PR.
**Before starting:** read `CLAUDE.md`, `ARCHITECTURE.md` (§6, §7, §8.4a,
§8.6 and §13), `DEVELOPMENT_STATUS.md` and the notes in
`docs/plans/M3_PLAN.md` §5.

## 1. Goal (ARCHITECTURE.md §13)

> **M4: Code transformations.** Translate, scale and rotate a selected
> range of program lines; the core rewrites the text (keeping comments
> and number style) and the editor applies it as one undoable edit.

Added by the owner (2026-11-01): the app remembers the machine,
dialect and window layout between sessions; the lines selected in the
editor are
shown in bright yellow in the 3D view; a better app icon; and on Linux,
`.nc` files show the app's icon in the file manager (Dolphin) and open
in the app.

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
   message, or transformed with a warning, never changed silently;
6. the app starts with the machine, dialect and window layout of the
   last session;
7. selecting lines in the editor colours their segments bright yellow in
   the 3D view;
8. the app has a proper icon, and the Flatpak makes `.nc` files show it
   and open in the app.

**Out of scope for M4:**
- mirroring (wishlist, owner decision)
- scaling other than uniform in X and Y (owner decision): Z is never
  scaled, and per-axis factors are not offered
- a preview of the transformed path before applying (not needed, owner
  decision: one Undo reverts it)
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
   - Scale: one factor for X and Y, and a centre; Z is unchanged (owner
     decision).
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
translation does not change deltas). To translate a selection that
starts in G91, the first move into it gains the offset, and if the first
move after the selection is incremental it loses the offset, so the rest
of the program is unchanged (owner decision). That second edit is outside
the selection; the message says so and names the line.

**Arcs (G2/G3).**
- I/J/K are offsets from the start (Fanuc default), so they are rotated
  and scaled with the move and unchanged by a translation.
- R is unchanged by translation and rotation and scaled with X and Y (a
  zero or negative factor is refused: a negative one would mirror).
- Scaling is in X and Y only, so an arc in the XZ or YZ plane (G18,
  G19) cannot be scaled exactly and is refused.
- An arc whose plane is not the rotation plane is refused (for example,
  a G18 arc rotated about Z), because it would leave its plane.
- Helical arcs: the axis outside the plane is translated like a line;
  rotation is about that axis and scaling leaves Z alone, so neither
  changes it.

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

**Refused, with a message** (owner decision): a selection containing
G53, G28/G30 intermediate points, canned cycles, G92/G52/G10, macro
variables or expressions, or a block the dialect reported as an error.
The whole transformation is refused, the message names the first such
line, and nothing is changed.

## 4. PR sequence

One PR per step, in this order. Each starts with failing tests. M4.1
and M4.2 come first, at the owner's request (2026-11-01): they make
every later test session quicker.

### M4.0: This plan
Agreed with the owner; ARCHITECTURE.md §13, CLAUDE.md and
DEVELOPMENT_STATUS.md point to it.

### M4.1: Remember the machine and dialect *(done)*
*Moved from the wishlist by the owner, 2026-11-01.*
- The app starts with the machine file and dialect chosen last, instead
  of the first machine in the list (which drew router programs mostly
  red, outside the generic VMC's travel).
- Kept in a small settings file in the app's configuration folder (next
  to `machines/`), read and written by `app-backend` (small JSON, not
  bulk data). The later settings dialog (wishlist) uses the same file.
- If the remembered machine file is gone or has problems, the app falls
  back to the first usable machine and says so in the status line.
- Tests: the settings file round-trips, a missing or damaged file gives
  the defaults (never an error at start-up), a removed machine falls
  back.
- **Where to start:** `app/shell/src/lib.rs` (`AppState`, `run()`'s
  `setup`, `select_machine`, `set_run_options`), `app/backend/src/files.rs`
  (the machine folder), and `ui/src/main.ts` (`fillMachines`,
  `applySettings`: today the first usable machine is chosen).

### M4.2: Remember the window layout *(done)*
*Moved from the wishlist by the owner, 2026-11-01.*
- The pane sizes (the dragged splitters), the maximised pane, and the
  window's size and maximised state are saved when they change and
  restored at start-up, in the same settings file.
- **Window position:** restored on X11 and on Windows and macOS. Under
  Wayland (the owner's desktop) an app cannot place its own window: the
  desktop decides, so only the size and maximised state come back. Said
  in the README.
- A layout that no longer fits (a smaller screen, a pane share out of
  range) is clamped or reset to the default, never restored off-screen.
- A way back to the default layout: a "Reset layout" item (the settings
  dialog on the wishlist would be its later home).
- Done with Tauri's own window API, not a new plugin, unless that proves
  unreliable (then `tauri-plugin-window-state`, justified in the PR).
- **Where to start:** `ui/src/layout/` (`addSplitter`, `Maximiser`;
  `panes.ts` holds the pure share and toggle logic), the window built in
  `app/shell/src/lib.rs` and `app/shell/tauri.conf.json`, and the
  settings file from M4.1.

### M4.2a: Remember the colour key and the diagnostics filters *(done)*
*Added by the owner, 2026-11-01, after M4.2 merged.*
- Whether the colour key in the view is folded, and which of the
  Errors / Warnings / Notes boxes in the diagnostics panel are ticked,
  are saved when the user clicks them and restored at start-up, in the
  same settings file (`legend_folded`, `hidden_diagnostics` in the
  layout). The editor's margin follows the restored filters, as it does
  when they are clicked.
- **Reset layout** sets them back to the defaults too (the key open,
  every box ticked).

### M4.3: The editor selection in the 3D view (UI) *(done)*
- Selecting lines in the editor colours all their segments **bright
  yellow** in the view (every run of a subprogram line, from the line
  index); a single cursor line is the one-line case of the same thing,
  replacing today's white highlight.
- Yellow is also the tool marker's and the measure line's colour; both
  are drawn on top in the overlay and keep their shapes, so they stay
  distinct. The colour key gains a "Selected" entry.
- Works for large selections (the whole of a million-block program):
  the highlight is built from index ranges, not segment by segment.
- Out of date (after an edit): no highlight, as for clicks today.
- Comes first because the transformations act on this same selection.

### M4.4: Exact rewriting (core)
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

### M4.5: Positions in program coordinates (core)
- For each block: modal state and the full programmed position before
  and after, in program coordinates and program units, from the
  interpreter (a per-block trace alongside the segments, only when
  asked, so normal runs pay nothing).
- Tests: absolute and incremental, units changes, work offset changes,
  subprogram calls.

### M4.6: Translate (core)
- G90 and G91 moves (the G91 rule in §3), arcs, helices, added words,
  the refusals.
- The check for all three transformations: run the original and the
  rewritten program; the rewritten selection's segments equal the
  original's transformed, within the rounding tolerance (property test).

### M4.7: Rotate (core)
- In G17, G18 and G19; added axis words; arcs and helices; incremental
  sections.

### M4.8: Scale (core)
- One factor for X and Y about a centre, Z unchanged; R and I/J; refusal
  of a zero or negative factor and of G18/G19 arcs.

### M4.9: The Transform dialog (app and UI)
- Shell command `transform` (raw program in, binary edit list out,
  parameters in small JSON) and its IPC tests.
- UI: Transform… dialog, the selected lines, centre picked from the
  view, applying the edits as one transaction, running again, showing
  warnings. Unit tests for the pure parts; a browser check.

### M4.10: A better icon
- The current icon is a placeholder. A new one as SVG (with the PNG
  sizes Tauri and the desktop need), drawn for clarity at 16–48 px.
- The agent proposes two or three designs as images; the owner picks
  (or supplies one). No trademarked logos or look-alikes.

### M4.11: `.nc` files on the Linux desktop (Flatpak)
- The Flatpak exports a MIME type for G-code (`*.nc`, `*.ngc`, `*.tap`
  and upper-case forms, in a package file named after the app id) with
  the app's icon, and the desktop file says the app opens it.
- **What a Flatpak cannot do:** it runs no script at install time, so it
  cannot make itself the default for a file type. Desktops (Dolphin
  included) use an app as the default when it is the only one for the
  type, or when the user picks "Always open with". The README says how.
- **A clash to handle:** the shared MIME database already gives `*.nc` to
  NetCDF (`application/x-netcdf`, binary scientific data). NetCDF files
  start with a fixed signature, so G-code can be told apart by content;
  the PR checks how Dolphin resolves it and chooses weights so G-code
  wins only for text files, without taking real NetCDF files.
- **The app must open a file given to it:** today it always starts with
  the sample. The shell passes a path from the command line (through the
  Flatpak document portal, `@@ %f @@`) to the UI, which opens it.
- Tested in the container's Flatpak and by the owner in Dolphin.

### M4.12: Wrap-up
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
| Claiming `*.nc` takes NetCDF files from scientific users | Match by content as well as name (M4.11); tested against a real NetCDF signature |

## 6. Owner decisions (2026-11-01)

1. **Translating incremental (G91) sections:** allowed. The first move
   after the selection is adjusted when it is incremental, and the
   message says so (§3).
2. **Mirroring:** on the wishlist, not in M4.
3. **Scaling:** one factor for X and Y only; Z is never scaled.
4. **Codes that cannot be transformed** in the selection: the whole
   transformation is refused, naming the line.
5. **Preview:** not needed.
6. **Added:** remembering the machine and dialect (M4.1) and the window
   layout (M4.2), moved from the wishlist to the start of M4; the editor
   selection shown bright yellow in the 3D view
   (M4.3), a better icon (M4.10), and `.nc` files showing the icon and
   opening in the app on the Linux desktop (M4.11).
