// Colouring the toolpath by tool (M5.5), as an alternative to colouring by
// move type. Each tool gets a colour from TOOL_PALETTE in the order the
// program first uses it, and its rapids a dimmed version of it. Moves
// outside the travel limits stay red and moves from an unknown position
// stay grey whatever the colouring, and the selected lines stay yellow
// (owner, M5_PLAN §6 item 3). Pure, so it is tested.

import { SEG_FLAG, SEG_KIND } from "../generated/render_buffer";
import { toolName } from "../panels/stats";
import { LEGEND, PALETTE, cssColor, srgbToLinear, type LegendEntry, type Rgb, type Toolpath } from "./toolpath";

import type { Colouring } from "../layout/panes";

/** The view's background (sRGB), which the palette must read well on. */
export const BACKGROUND: Rgb = [0x14 / 255, 0x17 / 255, 0x1c / 255];

const hex = (h: string): Rgb => [
  parseInt(h.slice(1, 3), 16) / 255,
  parseInt(h.slice(3, 5), 16) / 255,
  parseInt(h.slice(5, 7), 16) / 255,
];

/** Tool colours (sRGB), the most distinct first: each at least ΔE 25 from
 *  the others and 35 from red, grey and the selection's yellow, and at
 *  least 4.5:1 against the background (checked by the tests). */
export const TOOL_PALETTE: readonly Rgb[] = [
  "#4ea1ff", // blue
  "#2ee6c8", // teal
  "#86e05a", // green
  "#ff9f43", // orange
  "#c58cff", // purple
  "#ff70c4", // pink
  "#f2d0a9", // sand
  "#2a7bff", // deep blue
  "#b9f2ff", // ice
  "#9e9a3c", // olive
].map(hex);

/** The colour of the `index`th tool to be used (repeating after ten). */
export function toolColour(index: number): Rgb {
  const n = TOOL_PALETTE.length;
  return TOOL_PALETTE[((index % n) + n) % n] ?? [1, 1, 1];
}

/** A tool's rapid colour: halfway to the background. */
export function dimmed(c: Rgb): Rgb {
  return [0, 1, 2].map((i) => (BACKGROUND[i] ?? 0) + ((c[i] ?? 0) - (BACKGROUND[i] ?? 0)) * 0.5) as unknown as Rgb;
}

/** The colour of every vertex, in linear light: the toolpath's own (by
 *  move) or by tool, where `tools` lists the tools in order of first use. */
export function colourArray(
  tp: Pick<Toolpath, "colors" | "vertexSegment" | "segmentTool" | "segmentKind" | "segmentFlags">,
  colouring: Colouring,
  tools: readonly number[],
): Float32Array {
  if (colouring === "move") return tp.colors;
  const order = new Map(tools.map((t, i) => [t, i]));
  const linear = (c: Rgb): number[] => c.map(srgbToLinear);
  const feed = tools.map((_, i) => linear(toolColour(i)));
  const rapid = tools.map((_, i) => linear(dimmed(toolColour(i))));
  const red = linear(PALETTE.outOfLimits);
  const grey = linear(PALETTE.positionUnknown);
  const out = new Float32Array(tp.colors);
  for (let v = 0; v < tp.vertexSegment.length; v += 1) {
    const s = tp.vertexSegment[v] ?? 0;
    const kind = tp.segmentKind[s] ?? 0;
    const flags = tp.segmentFlags[s] ?? 0;
    let c: number[] | undefined;
    if ((flags & SEG_FLAG.OUT_OF_LIMITS) !== 0) c = red;
    else if ((flags & SEG_FLAG.POSITION_UNKNOWN) !== 0) c = grey;
    else if (kind === SEG_KIND.Rapid) c = rapid[order.get(tp.segmentTool[s] ?? 0) ?? 0];
    else if (kind === SEG_KIND.Linear || kind === SEG_KIND.ArcCw || kind === SEG_KIND.ArcCcw)
      c = feed[order.get(tp.segmentTool[s] ?? 0) ?? 0];
    if (c) out.set(c, v * 3);
  }
  return out;
}

/** The colour key: today's by move, or the tools with their colours. */
export function legendEntries(colouring: Colouring, tools: readonly number[]): readonly LegendEntry[] {
  if (colouring === "move") return LEGEND;
  const fixed = (label: string): LegendEntry[] => LEGEND.filter((e) => e.label === label);
  return [
    ...fixed("Selected"),
    ...tools.map((t, i): LegendEntry => ({ label: toolName(t), color: cssColor(toolColour(i)), style: "solid" })),
    { label: "Rapids: dimmed tool colour", color: cssColor(dimmed(toolColour(0))), style: "solid" },
    ...fixed("Outside travel limits"),
    ...fixed("Position unknown"),
    ...LEGEND.filter((e) => e.style === "dot"),
  ];
}

// Colour arithmetic for the palette's tests (CIE 1976 ΔE in CIELAB, and
// the WCAG contrast ratio), on sRGB colours.

function lab(c: Rgb): [number, number, number] {
  const [r, g, b] = c.map(srgbToLinear) as [number, number, number];
  const x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
  const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  const z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
  const f = (t: number): number => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))];
}

/** How different two colours look (CIE 1976 ΔE; about 2.3 is just
 *  noticeable, 25 clearly different). */
export function deltaE(a: Rgb, b: Rgb): number {
  const [l1, a1, b1] = lab(a);
  const [l2, a2, b2] = lab(b);
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

/** WCAG contrast ratio between two colours (1 to 21). */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const lum = (c: Rgb): number => {
    const [r, g, bl] = c.map(srgbToLinear) as [number, number, number];
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}
