# Machine definition files

A machine definition is a small text file describing your machine: its
axes, travel limits, speeds and work offsets. The simulator uses it to
interpret programs and to flag moves outside the machine's travel.

The files use [TOML](https://toml.io), a simple `key = value` format.
Lines starting with `#` are comments. A graphical editor for these files
is planned. Until then, copy the example and edit it in any text editor.

**Example:** [`core/machine-mill/data/generic_3axis_mill.toml`](../core/machine-mill/data/generic_3axis_mill.toml)

All lengths are in **mm**, angles in **degrees**, speeds in **mm/min**
(degrees/min for rotary axes) and spindle speed in **rpm**, whatever units
your programs use.

## `[machine]`

| Setting | Required | Meaning |
|---|---|---|
| `name` | yes | Display name |
| `family` | yes | Machine type that reads the file. Currently only `"mill"` (3-axis mill). |
| `max_feed` | no | Fastest cutting feed, mm/min |
| `max_spindle_rpm` | no | Fastest spindle speed, rpm |

## `[[axis]]`, one block per axis

List the axes in the order you want positions stored. Each axis is a
separate `[[axis]]` block.

| Setting | Required | Meaning |
|---|---|---|
| `letter` | yes | Address letter that programs the axis, e.g. `"X"`. |
| `kind` | yes | `"linear"` or `"rotary"`. |
| `direction` | no | `"x"`, `"y"` or `"z"`. For a linear axis, the direction it moves the tool. For a rotary axis, the axis it turns about. On a mill, X, Y and Z default to x, y and z, so this is only needed for other letters (e.g. U/V/W) or unusual layouts. |
| `arc_offset` | no | Letter giving arc-centre offsets along this axis (usually `"I"`, `"J"`, `"K"` for X, Y, Z). Without it, the axis cannot be part of an arc plane. Linear axes only. |
| `travel` | no | Soft limits in machine coordinates: `[min, max]`. Moves outside are flagged. |
| `rapid` | no | Rapid traverse rate, mm/min. |
| `diameter_mode` | no | `true` if this axis can be programmed as a diameter (turning). Default `false`. A mill rejects `true`. |

Every axis letter and arc-offset letter must be different.

## `[work_offsets]`

Where each work coordinate system's zero point is, in machine coordinates.
Number `1` is the first work offset (`G54` on Fanuc), `2` the second
(`G55`), and so on. Axes you leave out are 0.

```toml
[work_offsets]
1 = { X = -380.0, Y = -205.0, Z = -400.0 }
2 = { X = -200.0, Y = -205.0, Z = -400.0 }
```

Work offsets not listed here are reported when a program selects them.

## Errors

If the file has a mistake (a misspelled setting, a letter used twice,
`travel` with min above max, and so on), loading stops with a message
naming the problem. Unknown settings are rejected rather than ignored, so
a typo can't silently do nothing.

## 3-axis mill rules (`family = "mill"`)

- Exactly three linear axes, moving along x, y and z (one each).
- No rotary axes or diameter mode yet.
- The tool always points along +z.

The simulator is a visualisation aid. A program that stays within these
limits in the simulator is not thereby safe to run on the machine.
