# Wishlist

Ideas we want to keep but have **not** put in the development plan yet.
Nothing here is scheduled or promised. When an item is picked up, it moves
into a milestone plan (`docs/plans/`) and ARCHITECTURE.md §13, and is
removed from this list.

Add an entry with the date, who raised it, and why it matters. Keep each
one short; the design happens when it is planned.

## Settings dialog

*Raised by the owner, 2026-10-31.*

A dialog where the user sets the program's options, kept between sessions.
The first setting it would hold:

- **When the toolpath updates after an edit:**
  - *on refresh only* (the default since M2.8a): edits mark the view out of
    date, with a red border and a **Refresh (F5)** button, which suits
    large programs and big edits;
  - *automatically*, after a short pause in typing (how M2.6 worked),
    which suits small programs and fast computers.

Other options that could live there (to be decided when it is planned):
arc tolerance, block delete, the dialect's decimal-point setting, colours,
and the default machine and dialect.

## Remember the last machine and dialect

*Raised by the owner, 2026-10-31.*

The app starts on the first machine in the list (the generic VMC) every
time, so a router program opened straight away is drawn mostly red
(outside that machine's travel). Remember the machine and dialect chosen
last, and start with them. Fits the settings dialog above, which would
keep its options between sessions in the same way.

## Colour themes

*Raised by the owner, 2026-10-31.*

Let the user customise every colour, of the interface and of the toolpath
(rapids, feeds, arcs, out-of-limits, events, the tool marker, the
measurement line, the background), grouped into **themes**. Ship a dark
theme (today's colours) and a light theme as built-in options; users can
copy one and change it. Fits the settings dialog above. Today the
toolpath colours are in one place (`PALETTE` in
`ui/src/viewport/toolpath.ts`), and the colour key reads from it, so a
theme would replace that table; the interface colours are in
`ui/src/style.css` and would become CSS variables.

## Coordinate shifts and program coordinates

*Put on hold by the owner, 2026-10-31 (was M3.5); left to the community.*

Today G92, G52, G10, rotation (G68), scaling (G51), mirroring (G51.1),
polar input (G16) and Osai presetting (G92/G98) and per-axis origins
(`(UAO,n,axes)`) are ignored with a warning, and later moves are drawn
without their effect. A contributor implementing them would need:

- **Frames:** the program zero each segment was programmed in (work
  offset plus shifts), carried to the UI in a render buffer v2 (a small
  frame table and an index per segment; version bump, new fixture,
  regenerated constants), so the readout can show program coordinates
  with a Program/Machine toggle.
- **G92** relative to the machine's position when it runs (the machine
  file's reference position, at program start), and what cancels it
  (G92.1, reset, reference return: controller-dependent).
- **G52, G10 L2/L20,** rotation, scaling, mirroring, polar input, sourced
  from a manual for the control, or marked *(unverified)*.
- Care: on some machines (the owner's gang router) work offsets select
  spindles rather than move the part, so an option to keep drawing in
  program coordinates matters.

## Canned cycles

*Put on hold by the owner, 2026-10-31 (was M3.6); left to the community.*

Today each hole of G73/G74/G76/G81–G89 (Osai G81–G89) is drawn as a rapid
to its position in the working plane. A full implementation would expand
each cycle into rapids and feeds (approach, R level, feed in, pecks,
dwell, retract to the initial or R level per G98/G99), tagged with the
IR's `CycleStep`, all pointing back at the cycle's line.

## Macro variables and control flow

*Left out of M3.7 (2026-10-31): the owner's programs do not use them.*

Fanuc custom macro B (`#100 = 5`, `#[#1+2]`, `IF`/`WHILE`/`GOTO`, G65
calls with arguments) and the Osai equivalents. The parser already reads
expressions into a tree (ARCHITECTURE.md §7.4); evaluation would live in
a per-dialect `MacroEnvironment`, with the interpreter following jumps
as it follows subprogram calls. Today a block using a variable or an
expression is reported and skipped.


## Remember the window layout

*Asked for by the owner, 2026-10-31.*

Keep the pane sizes (dragged splitters) and which pane is maximised
between sessions, along with the window's own size and position. The
settings dialog above would be a natural home for a "reset layout"
button.
