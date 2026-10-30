%
O2000 (INTERPRETER FEATURES)
G21 G17 G40 G49 G80 G90 G94
T2 M06
G54 G00 X0. Y0. S2000 M03
Z5.
G01 Z-1. F150.
(R arcs: short and long way round)
G02 X20. Y0. R10.
G03 X0. Y0. R-10.
(full circle and helix)
G02 I10.
G03 X0. Y0. Z-3. I10.
(incremental square)
G91 G01 X10.
Y10.
X-10.
G90 G00 Z5.
(second work offset)
G55 X0. Y0.
(inch mode)
G20 G00 X1. Y1.
G21
(beyond the X travel limit: G55 X origin is -200)
G00 X250.
M05
M30
G00 X0.
%
