# M5 plan: everyday use, Osai level with Fanuc, and packages

**Status: agreed (owner, 2026-11-02); M5.1-M5.6g done (2026-11-03), M5.7 next.** The owner set
the content of M5 at the end of M4 (2026-11-02) and moved the earlier
milestones back by one: mobile is now M6, machine state and accuracy M7,
material removal M8, more dialects and machine types M9+
(ARCHITECTURE.md §13). The owner agreed the plan and its order when
starting M5 (2026-11-02); their decisions are in section 6.
**Before starting:** read `CLAUDE.md`, `ARCHITECTURE.md` (§7, §8.4a, §8.6,
§8.7, §13), `DEVELOPMENT_STATUS.md` (§2 working agreement, §7 practical
lessons) and `docs/plans/M4_PLAN.md` (what M4 built and decided).

## 1. Goal

Make the app nicer for daily work on real programs, bring the Osai 10
Series dialect level with Fanuc, and make the app easy to install on every
desktop platform.

The owner's list (2026-11-02):

1. **Filter the toolpath by tool:** a dropdown listing every tool in the
   program, with a checkbox for each; right-clicking a tool shows only
   that one.
2. **Find and Replace** in the editor, plain text (not G-code aware), and a
   simple find on Ctrl+F that repeats with F3.
3. **Packages:** if possible, builds for `.deb`, `.rpm`, Windows and macOS;
   the Flatpak file easy to find at the top of the repository; install
   instructions in the README for each platform.
4. **Colour by tool:** in the colour key, a choice between colouring by
   move type (today) and by tool, each tool its own colour and its rapids
   a dimmed version of it.
5. **Bug:** the red border that says the 3D view is out of date is hidden;
   it shows only while the panes are being resized.
6. **Performance:** holding Page Up/Down in the editor is slow, and it keeps
   scrolling after the key is let go; the same with Shift+Page Up/Down. The
   3D view is probably updated too often; about twice a second is enough.
7. **Osai level with Fanuc:** anything built so far that works only for
   Fanuc must work for the Osai 10 Series dialect too.

On the wishlist (owner, 2026-11-02): a **timeline player** that draws the
toolpath over time and moves the editor cursor in step, working with the
tool filter (hidden tools are skipped). See `docs/WISHLIST.md`.

**Done when:**

1. the toolpath can be filtered by tool (checkboxes, right-click for one
   tool), and coloured by tool with dimmed rapids, as an alternative to
   colouring by move type;
2. Find and Replace is easy to find, matches the app's look, and Ctrl+F
   and F3 (Shift+F3 back) work;
3. the out-of-date border is always visible while the view is out of date;
4. holding Page Up/Down (with or without Shift) scrolls smoothly and stops
   when the key is released, on the million-block benchmark program;
5. every feature works for Osai programs as it does for Fanuc ones, or the
   difference is agreed and documented (`docs/dialect-notes/osai.md`);
6. there are packages for the agreed platforms, with install instructions
   in the README, and the Flatpak is easy to find.

**Out of scope for M5:** the timeline player (wishlist), mobile (M6), tool
geometry, cutter compensation and acceleration (M7), material removal (M8),
other dialects and machine types (M9+), and the coordinate shifts and
canned cycles that are on hold.

## 2. What exists already (checked 2026-11-02)

- **Find and Replace is already in the editor**, from CodeMirror's search
  extension (`searchKeymap` in `ui/src/editor/editor.ts`): Ctrl+F opens a
  panel at the bottom of the editor with find, next, previous, all, match
  case, regular expression, by word, replace and replace all; Enter and F3
  go to the next match. It is plain text, as asked. It is easy to miss (no
  button, no menu) and drawn in CodeMirror's light default style, which
  does not match the dark editor. So M5.3 is mostly making it visible and
  matching the theme.
- **Every segment carries its tool** in the render buffer (`u16 tool`,
  ARCHITECTURE.md §8.3), so the tool filter and colouring by tool need no
  change to the core or the buffer: they are UI work.
- **The out-of-date state** is a `stale` class on the view, panel and
  editor boxes (`ui/src/main.ts`, `showFreshness`); the canvas probably
  covers the view's border, which is redrawn only while resizing.
- **Cursor movement** redraws the selection highlight once per frame
  (M4.3, `scheduleSelection`) and moves the tool marker and status on every
  line change (`showLine`).
- **Packaging:** only the Flatpak exists (CI builds it and keeps it as a
  download for 30 days). Tauri's bundler can make `.deb`, `.rpm` and
  AppImage files on Linux, an installer (`.msi`/`.exe`) on Windows and an
  `.app`/`.dmg` on macOS, each built on that system.

## 3. Rules and design notes

- **The UI holds no G-code meaning** (CLAUDE.md): the tool filter and the
  tool colours work from the render buffer's tool numbers, not from
  reading T words.
- **Hidden tools:** their segments are not drawn, cannot be picked or
  snapped to, and the selection highlight leaves them out; the editor is
  unchanged. The statistics and time estimate always cover the whole
  program (section 6 item 2).
- **Tool colours** must stay distinct from the safety colours: moves
  outside the travel limits stay red and moves from an unknown position
  stay grey whatever the colouring (section 6 item 3), and the selected
  lines stay bright yellow.
- **Settings:** the colouring choice is remembered in the same settings
  file as the layout (M4.2); the hidden tools are not remembered
  (section 6 items 2 and 3).
- **Throttling the view while scrolling:** while the cursor moves faster
  than a few lines a second (keys held), the highlight, marker and status
  update at most about twice a second, and once more right after the last
  movement, so the view always ends up showing the final line.
- **Osai level with Fanuc:** "level" means every app feature (drawing,
  statistics and time estimate, transformations, file handling, settings)
  works for Osai programs, within what the Osai stub reads. The audit
  (M5.6) lists each feature with its state for both dialects; gaps are
  fixed or agreed as differences. Behaviour that differs between the
  controls follows the Osai manual, cited in `docs/dialect-notes/osai.md`.
- **Packages** are built by CI; changing the workflows needs the owner's
  approval at that step (CLAUDE.md rule 9). Windows and macOS builds would
  be unsigned unless the owner wants to buy signing certificates, so the
  systems warn on first start; the README says how to open them anyway.

## 4. PR sequence (proposed order)

The two everyday annoyances first, since they are small and affect every
session; then the editor and display features; then Osai; packages last,
so that the packages carry everything else.

### M5.0: This plan
Agreed, with the owner's decisions in section 6 (2026-11-02). ARCHITECTURE.md §13,
CLAUDE.md, the README and DEVELOPMENT_STATUS.md point to it.

### M5.1: The out-of-date border always visible (bug)
- A failing browser check first: after an edit, the border is visible
  without resizing.
- Draw the border above the canvas (or as an outline the canvas cannot
  cover).

### M5.2: Smooth scrolling with Page Up/Down (performance)
- Measure first: holding Page Up/Down and Shift+Page Up/Down on the
  million-block program (`tools/bench --write`), with the CDP profiler,
  before and after.
- Throttle the 3D view's updates while the cursor moves quickly (section
  3); pure, tested throttling logic, wired into `main.ts`.
- **As built (#68 to #72 and the wrap-up):** the throttle did not fix it
  on the owner's machine, and was replaced: the highlight is a shader over
  the toolpath already on the GPU (#70, as PrusaSlicer's libvgcode does),
  held cursor keys are paced by the app (#72), and a diagnostics panel
  (Ctrl+Shift+D) reports timings and the graphics setup. The remaining
  cost is WebKitGTK copying frames through the CPU on NVIDIA; the same UI
  is fast in Chrome, so the owner chose to wait for Tauri's Chromium
  runtime (DEVELOPMENT_STATUS §8, wishlist).

### M5.3: Find and Replace
- Make CodeMirror's panel easy to find (a **Find…** button, with Ctrl+F
  and Ctrl+H shown in its tooltip) and draw it in the app's dark style.
- A found match is scrolled to about the middle of the editor, not the
  bottom edge (owner, section 6 item 4).
- Check Ctrl+F, Enter, F3, Shift+F3, replace and replace all, also with
  the panel closed (F3 repeats the last search); a replacement marks the
  view out of date like any edit.

### M5.4: Filter the toolpath by tool
- A **Tools ▾** dropdown in the toolbar (or the view): one checkbox per
  tool in the program, in the order they are first used, with **All** and
  **None**; right-clicking a tool shows only that one.
- Hidden tools' segments are left out of drawing, picking, snapping and the
  selection highlight; the drawing is rebuilt from index lists, so it stays
  fast on large programs.
- Unit tests for the pure parts (tool list, filtered index lists), and a
  browser check.

### M5.5: Colour by tool
- In the colour key: **By move** (today) or **By tool**. By tool, each
  tool gets its own colour from a palette chosen to be told apart on the
  dark background (and by colour-blind users as far as possible); its
  rapids are a dimmed version; the key lists the tools with their colours.
- Unit tests for the palette (distinct, readable, dimmed rapids) and the
  colouring; a browser check.
- **Added by the owner (2026-11-03): hiding rapids**, with its own toggle
  button in the 3D view, easy to switch on and off. Hidden rapids are
  treated like hidden tools (not drawn, not picked, snapped or measured
  to, left out of the highlight); not remembered, so the app always
  starts showing them.

### M5.6: Useful diagnostics, and Osai level with Fanuc
**Goal, set by the owner (2026-11-03):** the warnings and errors should be
about the *program*, like an arc whose radius cannot fit after a missing
positioning move, not about what the software does not support. Each
message that only says "not supported" is removed by supporting the code,
or, where the owner decides it need not be supported, by letting the
machine file declare it as having no effect, which silences it. Unknown
codes (typos, codes never seen) are still reported. The audit (in
`docs/dialect-notes/osai.md`, "Parity with Fanuc") lists every message
the owner's programs give today and what M5.6 does about each.

The new rule (CLAUDE.md, ARCHITECTURE.md §7.3a): *codes the dialect does
not know always produce a diagnostic; codes the machine file lists in
`ignore_codes` are recognised and produce none (the owner's
responsibility for that machine); a code that changes later moves but is
not simulated keeps one warning until it is supported or listed.*

- **M5.6a: This plan, the rule change and the audit** (docs only).
- **M5.6b: The machine file names its dialect and its power-on state.**
  `dialect = "osai"` in `[machine]`: choosing the machine sets the dialect
  (the dialect list stays, to override); the shipped generic 3-axis router
  becomes an Osai machine (the owner's router is an Osai control), the mill
  stays Fanuc. A `[power_on]` section (units, plane, motion mode) states how
  the control starts up, so the "assumed G21/G17/G00/G71" warnings are not
  given for a machine that states them.
- **M5.6c: `ignore_codes` silences, and takes G codes and Osai commands.**
  A listed code gives no message at all (today M codes get one note each).
  Besides M codes, a machine may list G codes and Osai three-letter
  commands (for example `G43`, `UAO`, `DWT`): their behaviour is unchanged
  (G43's Z move is still drawn, UAO and DWT stay unsimulated), only the
  message goes. M codes keep being removed from the block, as now.
- **M5.6d: Severities.** Cutter compensation (G41/G42): one warning per
  program, not one per block. M6 with no T word: a note, not a warning.
  G43 without being listed keeps its note.
- **M5.6e: Presetting (G92) supported, Fanuc and Osai.** Later moves are
  drawn with its effect (every program of the owner's Fanuc machine
  starts with `G54 G92 …`). Part of the coordinate shifts that were on
  hold; the rest (G52, G10, rotation, scaling) stays on hold.
- **M5.6f: Wrap-up of the audit.** The owner's programs, run through
  `tools/corpus` with their machine files, give no "not supported"
  messages; the dialect notes are updated.
- **M5.6g: The marker points along the move** (owner's request,
  2026-11-03, added before packaging). While the pointer is snapped to a
  node, the yellow arrow points the way the tool was travelling when it
  reached the node (the move arriving; at the very first point, its first
  move; on an arc, its tangent). Elsewhere (the editor's cursor, a clicked
  point) it stays vertical, along Z. A move straight toward or away from
  the viewer shows the ring, as Z does in the Top view.

Each fix starts with a failing test, conformance tests one file per
behaviour. Osai features no program of the owner's uses (subprograms,
helices, G16, labels, the parser's lexical differences) stay for M9,
listed in the audit.

### M5.7: Packages
- Linux `.deb` and `.rpm` (and AppImage if wanted), a Windows installer
  and a macOS app, built by CI with Tauri's bundler on each system (with
  the owner's approval of the workflow changes).
- All packages, the Flatpak included, published as a GitHub Release by CI
  with fixed "latest" download links in the README (section 6 item 5),
  and install instructions in the README for every platform. Linux
  first, then Windows, then macOS.
- Tested: the Linux packages installed in the container; Windows and macOS
  by the owner (or a CI smoke test that the app starts).

### M5.8: Wrap-up
- The owner's programs through every new feature; README, ARCHITECTURE.md,
  DEVELOPMENT_STATUS.md; packages to try.

## 5. Risks

| Risk | Mitigation |
|---|---|
| Tool colours confused with the safety colours (red, grey) or the selection (yellow) | Keep those colours whatever the colouring; test the palette against them |
| Filtering a million segments is slow | Rebuild index lists only when the filter changes, not per frame |
| Throttling makes the view feel laggy | Always update right after the last movement; tune with the owner |
| Osai parity grows large (the lexical profile is a parser change) | Audit first; the owner chooses what is in M5; the rest moves to M9 |
| Unsigned Windows and macOS builds alarm users | Say so in the README, with the steps to open them; signing only if the owner wants it |
| Binaries in the repository make it grow with every version | A GitHub Release instead (section 6 item 5) |

## 6. Owner decisions (2026-11-02)

1. **Order:** as proposed; reordering is fine if it helps.
2. **Tool filter:** the statistics and time estimate always cover the
   whole program, whatever the filter. The hidden tools are **not**
   remembered: every session (and every file) starts with all tools
   shown.
3. **Colour by tool:** moves outside the travel limits stay red and moves
   from an unknown position stay grey whatever the colouring; the choice
   between colouring by move and by tool is remembered with the layout.
4. **Find:** CodeMirror's panel at the bottom of the editor is fine; it
   needed a button, because it was not clear it existed. **Added:** a
   found match is scrolled to about the middle of the editor, not the
   bottom edge (M5.3).
5. **Packages:** as proposed: a GitHub Release published by CI, with
   fixed "latest" download links in the README, not binaries committed to
   the repository. Platforms in order of importance: Linux, Windows,
   macOS.
6. **Osai:** the owner's Osai programs use dwells, not subprograms,
   helices or labels; M5.6 closes the dwell gap (`DWT`) first. *(Changed on
   2026-11-03, item 7: `DWT` is listed in the machine file instead.)*
7. **M5.6 refocused (2026-11-03):** messages about unsupported codes are
   removed by supporting the code or by listing it in the machine file's
   `ignore_codes`, which silences it; G43, UAO and DWT are silenced only
   when listed; cutter compensation gives one warning per program; M6
   without T is a note; G92 is supported; the machine file names its
   dialect (choosing the machine sets it; the generic router is Osai) and
   its power-on state.
