# Golden tests

`*.nc` G-code inputs with reviewed expected outputs (see `ARCHITECTURE.md`
§11). Each pipeline stage has its own expected file next to the input:

- `<name>.syntax.txt`: the `gcode-syntax` parse result, checked by
  `core/gcode-syntax/tests/golden.rs`.
- `<name>.ir.json`: the interpreted result with the Fanuc dialect on the
  generic 3-axis mill (`core/machine-mill/data/generic_3axis_mill.toml`),
  as a compact summary, checked by `core/interpreter/tests/golden.rs`.

Do not regenerate expected files to make a failing test pass. If output
changes on purpose, regenerate with
`UPDATE_GOLDEN=1 cargo test -p gcode-syntax --test golden` (or
`-p interpreter`) and explain the diff in the PR.
