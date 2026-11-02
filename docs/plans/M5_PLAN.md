# M5 plan: everyday use, Osai level with Fanuc, and packages

**Status: agreed (owner, 2026-11-02); M5.1 in progress.** The owner set
the content of M5 at the end of M4 (2026-11-02) and moved the earlier
milestones back by one: mobile is now M6, machine state and accuracy M7,
material removal M8, more dialects and machine types M9+
(ARCHITECTURE.md §13). The owner agreed the plan and its order when
starting M5 (2026-11-02); the answers to section 6 are recorded there.
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
  unchanged. Whether the statistics follow the filter is a question
  (section 6).
- **Tool colours** must stay distinct from the safety colours: moves
  outside the travel limits stay red and moves from an unknown position
  stay grey whatever the colouring (proposed, section 6), and the selected
  lines stay bright yellow.
- **Settings** (the hidden tools, the colouring choice) go in the same
  settings file as the layout (M4.2), if they are to be remembered.
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
The owner agrees the order and answers section 6. ARCHITECTURE.md §13,
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

### M5.3: Find and Replace
- Make CodeMirror's panel easy to find (a **Find…** button, with Ctrl+F
  and Ctrl+H shown in its tooltip) and draw it in the app's dark style.
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

### M5.6: Osai level with Fanuc
- First an audit, in the PR description and `docs/dialect-notes/osai.md`:
  every feature, its state for Fanuc and Osai, and what is missing.
  Candidates from the notes: subprograms (Fanuc M98/M99 are followed; the
  Osai stub has none), dwell times (Osai's `DWT` is not read), helices
  (Osai's K pitch is refused), G16 planes by axis name, G79 only with G00,
  and the parser's Fanuc-shaped lexical rules (labels, E parameters,
  lower-case `r` and `t`, quoted strings in parentheses).
- The owner decides which gaps to close in M5; each fix starts with a
  failing test, with conformance tests one file per behaviour. This may be
  more than one PR.
- The owner's Osai programs (private, `tools/corpus`) stay clean.

### M5.7: Packages
- Linux `.deb` and `.rpm` (and AppImage if wanted), a Windows installer
  and a macOS app, built by CI with Tauri's bundler on each system (with
  the owner's approval of the workflow changes).
- The Flatpak where the owner chose (section 6), and install instructions
  in the README for every platform.
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
| Binaries in the repository make it grow with every version | Section 6, question 5 |

## 6. Questions for the owner

1. **Order:** the bug and the scrolling first (M5.1, M5.2), then Find,
   the tool filter, colour by tool, Osai and packages. Agreed, or another
   order?
2. **Tool filter:** should the statistics and time estimate follow the
   filter (only the shown tools), or always cover the whole program?
   Should the hidden tools be remembered between sessions?
3. **Colour by tool:** keep moves outside the travel limits red and moves
   from an unknown position grey whatever the colouring (proposed)? Should
   the choice be remembered (proposed: yes, with the layout)?
4. **Find:** is CodeMirror's panel at the bottom of the editor fine, made
   visible and dark (proposed), or would you prefer a separate dialog?
5. **Packages:** committing the `.flatpak` (and other installers) to the
   top of the repository adds a new copy to the history with every update
   (2.5 MB for the Flatpak, more for the others), so the repository grows
   for good. The alternative is a GitHub Release with fixed "latest"
   download links in the README: just as easy to find, without the growth,
   and CI can publish it. Which would you like? Which platforms matter most
   (Windows and macOS builds need their own CI runners and would be
   unsigned)?
6. **Osai:** your two Osai programs already draw cleanly. Do your other
   Osai programs use subprograms, dwells, helices or labels? That decides
   which gaps M5.6 closes first.
