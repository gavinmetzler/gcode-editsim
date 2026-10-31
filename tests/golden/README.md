# Golden tests

`*.nc` G-code inputs with reviewed expected outputs (see `ARCHITECTURE.md`
§11). Each pipeline stage has its own expected file next to the input:

- `<name>.syntax.txt`: the `gcode-syntax` parse result, checked by
  `core/gcode-syntax/tests/golden.rs`.
- `<name>.ir.json`: the interpreted result with the Fanuc dialect on the
  generic 3-axis mill (`core/machine-mill/data/generic_3axis_mill.toml`),
  as a compact summary, checked by `core/interpreter/tests/golden.rs`.

Files named `router_*.nc` are short excerpts of the owner's real router
programs (published with the owner's agreement, comments removed). They run
on the generic router (`core/machine-mill/data/generic_3axis_router.toml`:
no travel limits, every work offset at zero) instead of the generic mill.

`render_buffer_v1.bin` is the shared render-buffer fixture (ARCHITECTURE.md
§8.4). It is built from a hand-made program in
`core/ir/tests/render_buffer.rs`, which also lists every value it holds;
the TypeScript decoder test checks the same values. Regenerate it with
`UPDATE_GOLDEN=1 cargo test -p ir --test render_buffer` only together with
a layout version bump.

Do not regenerate expected files to make a failing test pass. If output
changes on purpose, regenerate with
`UPDATE_GOLDEN=1 cargo test -p gcode-syntax --test golden` (or
`-p interpreter`) and explain the diff in the PR.
