%
O3000 (REVIEW FINDINGS: TOOL CHANGE, CANNED CYCLE, ROTATION, FULL CIRCLE)
G21 G17 G40 G49 G80 G90 G94
T1 M06
G54 G00 X0. Y0. S2000 M03
G43 H1 Z5.
G01 Z-2. F200.
(incremental steps, then a full circle back at the start)
G91 X0.1
X0.1
X0.1
G90 G02 X0.3 Y0. J5.
G00 Z5.
(standard Fanuc tool change: retract Z, then X and Y, to the reference)
G91 G28 Z0.
G28 X0. Y0.
G90
T2 M06
G54 G00 X50. Y0.
G43 H2 Z5.
(drilling cycle, holes drawn as rapids, cancelled by G00 instead of G80)
G99 G81 X50. Y0. Z-10. R2. F100.
X60.
G00 Z50.
X70. Y0. Z5.
(rotation: not simulated, so later moves are drawn without it)
G68 X0. Y0. R45.
G01 X80. F300.
G69
G00 X90.
X90. Y10. Z5.
X100.
M05
M30
%
