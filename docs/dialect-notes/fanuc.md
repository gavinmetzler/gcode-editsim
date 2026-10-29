# Fanuc dialect notes

Behaviour of `core/dialect-fanuc` where Fanuc controls differ by series,
parameter setting or machine builder, and where this simulator had to pick
one. Each entry says what we do, whether we warn, and where the behaviour
comes from.

**Source status.** Entries marked *(verified)* were checked against the
public sources listed. Entries marked *(unverified)* are from general
knowledge of the Fanuc 0i/16i/18i operator manuals and still need checking
against the manual for the target control. Please update the marker when you
verify one.

This simulator is a visualization aid. Nothing here means a program is safe
to run on a machine.

## Numbers without a decimal point (parameter DPI)

- **What we do:** by default, a length or angle written without a decimal
  point is read in *least input increments*. These are 0.001 mm (metric),
  0.0001 inch (inch), or 0.001° (rotary), so `X10` means 0.010 mm and
  `X10.` means 10 mm. `FanucOptions { decimal_input: Calculator }` switches
  to calculator-type input, where `X10` means 10 mm.
- **Scope:** this applies to axis words, I/J/K and R. F, S and T are never
  scaled.
- **Warning:** `ControllerDependent`, reported once per program, at the
  first non-zero value without a decimal point.
- **Source:** parameter 3401 bit 0 (DPI) on 0i/16i/18i-class controls (2400
  bit 0 on older 11/15 series). DPI = 0 is the standard notation (least
  input increment); DPI = 1 is calculator type. *(verified)*
  - [mfgrobots: Fanuc decimal point programming](https://www.mfgrobots.com/Article/equipment/cnc/46531.html)
  - [eMastercam forum: Fanuc 0i-TD parameter question](https://www.emastercam.com/forums/topic/82671-fanuc-0i-td-parameter-question/)
- **Assumption:** IS-B is the input increment system. This is by far the
  most common one. IS-C (0.0001 mm) is not modelled.

## Several G codes from the same modal group in one block

- **What we do:** the last one written wins, and the earlier ones are
  dropped.
- **Warning:** `ModalGroupConflict`, on every occurrence.
- **Source:** several third-party references describe Fanuc-style controls
  keeping the last same-group code. The LinuxCNC interpreter cites the
  Fanuc manual as allowing only one code per group per line, so the
  behaviour is worth flagging. *(verified, third-party)*
  - [Mach4 G and M code reference](https://machmotion.com/documentation/Software/Mach4/Mach4-G-and-M-Code-Reference-Manual.pdf)
  - [Helsinki hacklab G-code wiki](https://wiki.helsinki.hacklab.fi/G-koodi)
  - [LinuxCNC interp_array.cc](https://git.bues.ch/cgit/linuxcnc.git/tree/src/emc/rs274ngc/interp_array.cc?h=2.9)

## Arcs

- **No R and no I/J/K:** this is an error (`InvalidArc`), and the block does
  not move. Fanuc raises alarm 22 "R OR I,J,K COMMAND NOT FOUND".
  *(verified)*
  - [Fanuc 16i/18i alarm list](https://studylib.net/doc/8255427/fanuc-series-16i-160i-160is-modelb-series-18i-180i-180is)
  - [MRO Electric: Fanuc alarms](https://www.mroelectric.com/blog/fanuc-alarms/)
- **I/J/K are always offsets from the start point to the centre,** in both
  G90 and G91. *(unverified, but standard Fanuc milling behaviour)*
- **R and I/J/K in the same block:** R is used and the offsets are
  ignored. Reported as a `ControllerDependent` warning. *(unverified)*
- **R with no end point** (`G2 R5.`): Fanuc treats this as an arc of 0°,
  so nothing moves. Reported as an `InvalidArc` warning. *(unverified)*
- **I/J/K with no end point:** a full circle, following the IR rule that
  coincident start and end points mean a full circle. *(unverified, but
  standard)*
- **Negative R** selects the arc longer than 180°. Interpreted downstream.
  *(unverified, but standard for milling; lathe series alarm on negative R)*
- **An offset letter for an axis outside the selected plane** (e.g. K in
  G17) is ignored, with an `InvalidArc` warning. *(unverified)*
- **G17/G18/G19 choose the plane by axis letters** X/Y, Z/X and Y/Z. The
  axes are looked up in the machine's axis list, so the plane fails cleanly
  on a machine without those axes.

## Power-on modal state

G00, G17, G90, G94, G21, G40, G49, G80, G54, G98. *(unverified)*

These are factory-default parameter settings. On real controls:
- G00/G01 at power-on is set by parameter 3402 bit 0.
- The plane is set by 3402 bits 1–2.
- G90/G91 is set by 3402 bit 3.
- Units are whatever was active at power-off.

Programs normally set all of these explicitly in a safety line.

## Order of effects within a block

1. G-code modes (units first, so the block's own values use them).
2. F, S, T.
3. M3, M4, M6, M8.
4. Motion.
5. M0, M1, M2, M5, M9, M30.

This is the common convention. Fanuc lets machine builders choose whether an
M code acts with or after motion. *(unverified)*

## M codes

- **Defined by Fanuc:** only M00, M01, M02, M30, M98 and M99. Everything
  else is defined by the machine builder.
- **What we do:** M03/M04/M05 (spindle), M06 (tool change), M08/M09
  (coolant) follow the near-universal convention. M07 (mist) is not mapped
  because builders disagree. *(unverified)*
- **M02 vs M30:** M02 ends without rewinding and M30 ends and rewinds. On
  some controls M02 also rewinds, depending on a parameter. *(unverified)*
- **More than one M code in a block:** this needs the multiple M-code option
  (parameter 3404 bit 7, M3B). We apply them all in order and give one
  `ControllerDependent` warning per program. *(unverified)*

## Not supported yet

Each of these produces a diagnostic, never silence:

| Code(s) | Severity | Does the block still move? | Notes |
|---|---|---|---|
| G41/G42 (cutter compensation), G43/G44 (tool length compensation) | warning (`NotYetSimulated`) | yes | Shown without the compensation |
| G04, G10, G28, G53, G65, G92 | error | no | Their axis words don't mean "move here" |
| Canned cycles G73, G74, G76, G81–G89 | error | no | Every following block is also not moved until G80 |
| G95 (feed per revolution) | error | yes | The path is still drawn |
| M98/M99 (subprograms), any unknown G/M code | error | an unknown G code blocks motion | |
| Macro variables and expressions (`#100`, `[...]`) | error | no | The block is skipped |
| Macro assignments (`#100 = 5`) | warning | n/a | Has no effect |

## Repeated address in one block

- **What we do:** `X1. X2.` is an error and the block is skipped. This is
  the conservative choice, because Fanuc behaviour here was not confirmed.
  *(unverified)*
