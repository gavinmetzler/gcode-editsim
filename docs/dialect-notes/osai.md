# Osai 10 Series dialect notes

Behaviour of `core/dialect-osai`, a **stub** written in M2 to check that
the dialect interfaces suit a controller other than Fanuc. It covers a
small milling subset. Every app feature is to work for Osai programs as for
Fanuc ones in M5 (`docs/plans/M5_PLAN.md`, M5.6); full Osai support is
planned for M9 (ARCHITECTURE.md §13).

**Source.** Osai, *10 Series CNC Programming Manual* (2006/2007 edition),
cited below by page ("p. 2-4" is chapter 2, page 4). The manual is
copyrighted and is not part of this repository. Entries marked
*(verified)* follow what that manual states. Entries marked *(unverified)*
are choices the manual leaves open; they need checking on a real control.
The machine builder can change much of the default configuration (AMP),
so a real machine may differ even where the manual is clear.

This simulator is a visualization aid. Nothing here means a program is safe
to run on a machine.

## Program text

- **Comments start with `;`** and run to the end of the line (p. 1-2,
  1-4). *(verified)*
- **Parentheses hold three-letter commands, not comments**, e.g.
  `(UAO,1)` or `(DIS,"text")` (p. 1-4). The shared parser reads
  parentheses as comments; the dialect finds the commands there. A
  parenthesis that is not shaped like a command (e.g. a Fanuc-style
  `(TOOL 1)`) gets an `UnsupportedWord` warning and is ignored. What the
  control does with it is *(unverified)*.
- **Numbers are read as written**: `X10` is 10 mm, with or without a
  decimal point (the manual's examples, e.g. p. 2-4). *(verified)*
- **Syntax the shared parser cannot read**: labels (`"START"`, p. 1-3),
  axis names without a value (`G X Y`, p. 2-81), `>>` incremental marks
  (p. 2-80), E-parameter assignments (`E10=5`, p. 1-4) and settings such
  as `DWT=2`. Such a line gets a `SyntaxError`, and because the control may
  have moved there, **every axis's position becomes unknown**: moves are
  not drawn again until every axis has been given an absolute position.
  (The Fanuc dialect only skips such a line, since Fanuc programs rarely
  contain valid syntax the parser rejects.)
- **Lower-case letters**: the shared parser reads letters case-blind. On
  the 10 Series lower-case `r` rounds a corner (p. 2-28 example) and `t`
  sets a block time (p. 1-7), so they arrive as `R` and `T`. `R` outside
  an arc is therefore an error, and the position becomes unknown. A
  lower-case `t` is read as a tool selection *(known limitation)*.

## Power-up state (p. 1-11, 1-12, "POWER UP" column, milling)

- G00, G17, G27, G40, G71, G80, G90, G94, G97 and G99 are active.
  *(verified)*
- All axes are referred to the **home position** (`(UAO,0)`) until a
  `(UAO,n)` selects an origin (p. 2-95). Unlike Fanuc, there is no work
  origin at power-up. *(verified)*
- **Warning:** `ControllerDependent`, once per program, when a program
  relies on the default units (G71) or plane (G17) without selecting them,
  because the machine builder can change these defaults (p. 1-11, p. 2-79:
  "typically G71"). The absolute default (G90) is stated without that
  caveat (p. 2-80), so it is not warned about.

## Codes (code table `core/dialect-osai/data/osai.toml`)

| Code | What we do | Source |
|---|---|---|
| G00, G01 | Rapid, linear move; G01 without any F is an error and does not move | p. 2-2, 2-3 *(verified)* |
| G02, G03 | Arcs, see below | p. 2-4 *(verified)* |
| G17, G18, G19 | Plane of the 1st and 2nd, 3rd and 1st, 2nd and 3rd axes **in the machine's axis order**. Must be alone in the block. A warning if the first three axes are not X, Y, Z | p. 2-46, 2-47, 1-10 *(verified)* |
| G16 | Plane by axis names (`G16 X A`): the parser cannot read it, so arcs are not shown until G17-G19 | p. 2-48 |
| G27, G28, G29 | Continuous or point-to-point movement: no effect on the path. **G28 is not a reference return** | p. 2-49 *(verified)* |
| G40 / G41, G42 | No effect / cutter compensation not simulated (one warning per program, M5.6d; G93 and G96 likewise once each) | p. 1-11 |
| G70, G71 | Inches, millimetres | p. 2-79 *(verified)* |
| G20, G21 | GTL profiles, not units: unknown code | p. 1-11 *(verified)* |
| G79 | Coordinates refer to the home position for one block (machine coordinates here). Supported with G00 only; with a feed move, an error and the position becomes unknown | p. 2-80, 2-81 *(verified; the G00-only limit is the stub's)* |
| G80 / G81-G89 | Canned cycles not simulated (on hold): each hole is drawn as a rapid in the working plane, until G80; one warning per cycle code | p. 1-12. That only G80 ends them is *(unverified)* |
| G90, G91 | Absolute, incremental | p. 2-80 *(verified)* |
| G92, G98 / G99 | Presetting (M5.6e): the current position gets the programmed values (always absolute), and later moves are drawn shifted, under every origin; axes not named keep their shift. G98's mirroring is not simulated. G99, which must be alone in the block, and M02/M30 cancel it. An axis whose position is unknown cannot be preset (warning) | p. 2-82, 1-10 *(verified)* |
| G04 | Dwell at the end of the block. Its time comes from `DWT`, which is not read: the dwell is shown without its time (warning) | p. 2-83 *(verified)* |
| G09 | Deceleration only: no effect | p. 2-83 *(verified)* |
| G93 / G94 / G95 | Inverse-time feed not simulated (warning) / per minute / per revolution: F × S gives mm/min while the spindle turns; with it stopped the move has no time in the estimate (warning, M3.8) | p. 1-7, 1-12 |
| G96 / G97 | Constant surface speed not simulated: S is not taken as rpm (warning) / rpm | p. 1-8 |
| Others (G33, G60-G67, G72-G74, ...) | Unknown: an error, the block is not moved, and later moves are drawn as programmed | |

Two codes of one group in a block are an error and the block is skipped
(compatibility table p. 1-10). The rest of that table is not checked yet.

## Arcs (p. 2-4 to 2-6, 2-16)

- **I and J are the centre on the plane's first and second axis, whatever
  the plane** ("No matter what interpolation plane you are using, the
  symbol for the abscissa is always I"). The machine file's `arc_offset`
  letters are not used. *(verified)*
- **In G90, I and J are the centre's position** in the current origin; in
  G91 they are distances from the start (example on p. 2-5: from X10 Y20,
  `G2 X46 Y20 I28 J20` has its centre at X28 Y20). *(verified)* This is
  `ArcCenter::Position` in the dialect interface.
- In G90 an arc needs both I and J; with only one, the arc is not shown and
  the position becomes unknown *(unverified: what the control does)*. In
  G91 a missing I or J is a zero distance *(unverified)*.
- **No axes in the block** makes a full circle. *(verified)*
- **R** is the radius, negative for more than 180°. R cannot make a full
  circle (an error; the block does not move). *(verified)* R together with
  I/J is not shown and the position becomes unknown *(unverified)*.
- **On a diameter axis** I and J are diameters (p. 1-6, 2-4). The
  interpreter converts them like end points. *(verified)*
- **K is the helix pitch.** Not simulated: the arc is not shown and the
  position becomes unknown. *(known limitation)*
- **Tolerances:** the radius difference allowed between start and end
  (CET) defaults to 0.01 mm, as in the interpreter (p. 2-7). The
  full-circle threshold (FCT) defaults to 0.001 mm (p. 2-8); the
  interpreter closes circles only within 1e-6 mm, so an end point between
  the two is drawn as a short arc instead of a full circle *(known
  difference)*.
- **Direction:** clockwise and counter-clockwise follow the usual
  right-hand convention for the plane, as for Fanuc. The manual's figure
  is not legible in our copy *(unverified)*.

## Three-letter commands (p. 1-4, appendix A-7)

| Command | What we do |
|---|---|
| `(UAO,n)` | Origin n (0-10) for all axes; 0 is the home position (p. 2-95). Origin n is work offset n in the machine file. |
| `(UAO,n,axes...)` | Origin n for the named axes only (p. 2-95). **Not simulated; ignored** at the owner's request (2026-10-31; coordinate shifts are on hold): a warning, the current origin stays in use, and moves keep being drawn, so moves on the named axes may be offset. So is an origin number outside 0-10 |
| `(DIS,...)`, `(UGS ...)`, `(CGS)`, `(DGS)` | Messages and the graphics window: no effect on the path |
| Anything else (`ROT`, `SCF`, `MIR`, `UTO`, `UIO`, `CLS`, `GTO`, `RPT`, ...) | Not simulated: a warning, and later moves are drawn without its effect (moves are never hidden, owner decision 2026-10-31) |

## M codes (chapter 6, "Standard M functions")

The machine builder defines the M codes of a real machine; these are the
standard ones. Order within a block: M3, M4, M6, M7, M8, M9 and M19 act
before the move, M0, M1, M2, M5 and M30 after it. **M9 acts before the
move, unlike Fanuc.** *(verified for the codes the manual describes; M13
and M14 are taken to act like M3/M4 (unverified))*

| Code | What we do |
|---|---|
| M0, M1 | Stop, optional stop |
| M2, M30 | End of program (M30 also rewinds) |
| M3, M4, M5 | Spindle CW, CCW, stop |
| M6 | Tool change to the tool selected by T; without a T word, a note (the tool stays unknown; an error until M5.6d) |
| M7 | Auxiliary coolant: shown as mist (the IR has only flood and mist) |
| M8, M9 | Main coolant on, all coolant off |
| M13, M14 | Spindle CW / CCW and coolant on |
| M19 | Spindle stop (the orientation is not simulated) |
| Others | `UnsupportedMCode` error |

## T, S, F

- **T**: the digits before the point are the tool code, the digits after
  it the offset number (p. 1-8). `T9.9` selects tool 9; offsets are not
  simulated. *(verified)*
- **S**: rpm under G97. Under G96 it is a surface speed, which is not
  simulated. *(verified)*
- **F**: mm/min (G71) or inch/min (G70) under G94 (p. 1-7). *(verified)*

## Transformations (M4.9a)

What the code transformations (Translate, Rotate, Scale) do with 10 Series
programs. Our choices, not controller behaviour, except where cited.

- **Numbers are written as read**: a number with a decimal point keeps at
  least the decimals it had; one without stays without when the new value
  is whole, and gains a point when it is not, since `X12` means 12 mm
  (p. 2-4) *(verified: the reading rule)*. Values are rounded to 0.001 mm
  or degree and 0.0001 inch, or kept to the decimals the number already
  had if it had more. The manual allows up to 9 decimals and cuts off any
  more without an error ("Programming restrictions for long real
  formats", p. 1-15), and gives no input increment (the resolution is a
  machine setting): the rounding is *our choice*. The manual also says no
  zeros should precede or follow a value (p. 1-2); rewriting never adds
  them, and keeps a number's own style where a program has them.
- **Arc centres**: in G90, I and J are the centre's position, so they move
  with the selection like an end point (a translation moves them too); in
  G91 they are distances from the start, so they turn and scale like
  Fanuc's offsets. I and J name the plane's first and second axis in any
  plane. **R** scales with a scale factor and keeps its sign.
- **Refused** (the whole transformation, naming the line): presetting
  (G92, G98, G99), canned cycles (G81-G89, and every block while one is
  active), G79, and three-letter commands that change later coordinates:
  `(UAO,n,axes)` (an origin for some axes), `ROT`, `SCF`, `MIR`, `UTO`,
  `UIO` and any other not known to leave the path alone. A plain
  `(UAO,n)` changes the origin for every axis, like Fanuc's G54, and is
  allowed. Messages and graphics commands (`DIS`, `UGS`, …) are ignored.

## Parity with Fanuc, and the owner's programs (M5.6 audit, 2026-11-03)

Every app feature against both dialects, and what M5.6 does
(`docs/plans/M5_PLAN.md`, M5.6). "Programs" are the owner's private
programs (2 Osai, 4 Fanuc) run through `tools/corpus`.

| Feature | Fanuc | Osai | M5.6 |
|---|---|---|---|
| Drawing, arcs, travel limits, picking, measuring | yes | yes | - |
| Statistics and time estimate | yes, dwell times read (G04 X/P) | yes, but dwells have no time (`DWT` not read) | `DWT` silenced when listed in the machine file; times stay without dwells |
| Transformations (M4) | yes | yes | - |
| Tool filter, colour by tool, Find, files, settings | yes (no dialect involved) | yes | - |
| Presetting (G92) | supported (M5.6e) | supported (M5.6e) | done |
| Origins by axis (`UAO`) | (no equivalent) | not simulated (warning) | silenced when listed (M5.6c) |
| Tool length compensation (G43) | ignored (note) | ignored (note) | silenced when listed (M5.6c) |
| Cutter compensation (G41/G42) | warning per block | warning per block | one warning per program (M5.6d) |
| Assumed power-on modes | warnings | warnings | none when the machine file states its power-on state (M5.6b) |
| Machine-specific M codes | one note each | one note each | none when listed (M5.6c) |
| Subprograms | followed (M98/M99) | not read | M9: no program of the owner's uses them |
| Helices (K pitch), G16 by axis name, labels, E parameters, lower-case `r` and `t`, quoted strings | - | refused or not read | M9: not used by the owner's programs |

Messages the owner's programs give today (machine: the generic router):

| Message | Programs | M5.6 |
|---|---|---|
| Machine-specific M codes (M11, M13, M20, M21, M23, M40, M92, M93, M95, M211, M213, M400, M401), notes | all 4 Fanuc | listed in the owner's machine file (M5.6c) |
| Assumed G00 / G21 / G17 (Fanuc), G17 / G71 (Osai), warnings | all 6 | machine file power-on state (M5.6b) |
| G92 not simulated, warning | all 4 Fanuc | gone: supported (M5.6e) |
| G43 not simulated, note | all 4 Fanuc | listed in the machine file (M5.6c) |
| G42 cutter compensation, warning per block | 1 Fanuc | one warning (M5.6d) |
| M6 with no tool selected (`G92 … M06`), warning | 2 Fanuc | a note (M5.6d) |
| `(UAO,1,Z)` not simulated, warning | 1 Osai | listed in the machine file (M5.6c) |

`300sl_X1.nc` (Osai, 21 diagnostics in the owner's report) is not in the
private repository yet; to be audited when it is.
