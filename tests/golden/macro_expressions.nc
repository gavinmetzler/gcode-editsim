(Macro variables and expressions are parsed, not evaluated.)
#100 = 5
#101=#100*2 + 1
#[#1+1] = SIN[30.] * #2
G1 X#100 Y-#101 Z[#100/2]
G1 X[ATAN[1, 2]] Y#[#100+1] Z##3
#1 = [#2 GT 3] AND [#4 LE 5]
