# Osai 10 Series dialect notes

Behaviour of `core/dialect-osai`, a **stub** written in M2 to check that
the dialect interfaces suit a controller other than Fanuc. It covers a
small milling subset; full Osai support is planned for a later milestone.

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
| G40 / G41, G42 | No effect / cutter compensation not simulated (warning) | p. 1-11 |
| G70, G71 | Inches, millimetres | p. 2-79 *(verified)* |
| G20, G21 | GTL profiles, not units: unknown code | p. 1-11 *(verified)* |
| G79 | Coordinates refer to the home position for one block (machine coordinates here). Supported with G00 only; with a feed move, an error and the position becomes unknown | p. 2-80, 2-81 *(verified; the G00-only limit is the stub's)* |
| G80 / G81-G89 | Canned cycles not simulated: motion hidden until G80 | p. 1-12. That only G80 ends them is *(unverified)* |
| G90, G91 | Absolute, incremental | p. 2-80 *(verified)* |
| G92, G98 / G99 | Presetting not simulated: motion hidden until G99. G99 must be alone in the block | p. 2-82, 1-10 *(verified)* |
| G04 | Dwell at the end of the block. Its time comes from `DWT`, which is not read: the dwell is shown without its time (warning) | p. 2-83 *(verified)* |
| G09 | Deceleration only: no effect | p. 2-83 *(verified)* |
| G93, G95 / G94 | Inverse-time and per-revolution feed not simulated (warning) / per minute | p. 1-7, 1-12 |
| G96 / G97 | Constant surface speed not simulated: S is not taken as rpm (warning) / rpm | p. 1-8 |
| Others (G33, G60-G67, G72-G74, ...) | Unknown: an error, and motion is hidden to the end of the program | |

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
| `(UAO,n,axes...)` | Per-axis origins are not supported: motion hidden until a `(UAO,n)` for all axes |
| `(DIS,...)`, `(UGS ...)`, `(CGS)`, `(DGS)` | Messages and the graphics window: no effect on the path |
| Anything else (`ROT`, `SCF`, `MIR`, `UTO`, `UIO`, `CLS`, `GTO`, `RPT`, ...) | An error, and motion is hidden to the end of the program |

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
| M6 | Tool change to the tool selected by T |
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
