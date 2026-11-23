// The tool marker's shape on screen: a thick shaft with an arrowhead whose
// tip is the point, along the machine's Z axis as the view shows it.
// While the pointer is snapped to a node it is instead a short arrow
// starting at a white dot on the node and pointing the way the tool was
// travelling when it got there (M5.6g; shortened and turned round in M6,
// owner: an arrow should point away from where we are).
// Deliberately not a tool shape, so it is not mistaken for a simulated
// tool. Drawn as a flat overlay over the 3D view, so moving it never
// redraws the toolpath. Pure, so it is tested without a browser.

/** The marker's size on screen, in pixels, when Z lies in the view plane. */
export const MARKER = { length: 64, shaft: 5, headLength: 18, headWidth: 16, ring: 9 } as const;

/** Below this share of its full length (Z pointing at or away from the
 *  viewer, as in the Top view) the arrow is drawn as a ring instead. */
const MIN_FORESHORTENING = 0.25;

/** What to draw, in screen pixels. */
export type MarkerShape =
  | {
      readonly kind: "arrow";
      /** The shaft, from the head's base to the far end. */
      readonly shaft: readonly [number, number, number, number];
      /** The head: tip, then the two corners of its base. */
      readonly head: readonly [number, number, number, number, number, number];
    }
  | { readonly kind: "ring"; readonly x: number; readonly y: number; readonly r: number };

/**
 * The marker at screen point (`x`, `y`). (`zx`, `zy`) is how far, in
 * pixels, one mm along the shaft moves on screen (+Z, or back along the
 * travel: `tailOnScreen`); `pxPerMm` is the view's scale. The shaft runs
 * from the tip that way, shortened as it tilts toward the viewer.
 */
export function markerShape(x: number, y: number, zx: number, zy: number, pxPerMm: number): MarkerShape {
  const along = Math.hypot(zx, zy);
  const f = pxPerMm > 0 ? along / pxPerMm : 0;
  if (!(f >= MIN_FORESHORTENING) || !Number.isFinite(f)) {
    return { kind: "ring", x, y, r: MARKER.ring };
  }
  const ux = zx / along;
  const uy = zy / along;
  const k = Math.min(f, 1);
  const len = MARKER.length * k;
  const head = MARKER.headLength * k;
  const w = MARKER.headWidth / 2;
  const bx = x + ux * head;
  const by = y + uy * head;
  return {
    kind: "arrow",
    shaft: [bx, by, x + ux * len, y + uy * len],
    head: [x, y, bx - uy * w, by + ux * w, bx + uy * w, by - ux * w],
  };
}

/**
 * One mm along the marker's shaft, on screen in pixels: back along the
 * unit `travel` direction (so the arrow points along it), or +Z when there
 * is none or it is not a number. `m` is the projection times view matrix
 * (column-major, orthographic), the canvas `width` by `height` pixels.
 */
export function tailOnScreen(
  travel: readonly [number, number, number] | null,
  m: ArrayLike<number>,
  width: number,
  height: number,
): [number, number] {
  const ok = travel !== null && travel.every((c) => Number.isFinite(c));
  const [x, y, z] = ok ? [-travel[0], -travel[1], -travel[2]] : [0, 0, 1];
  const e = (k: number): number => m[k] ?? 0;
  const sx = ((e(0) * x + e(4) * y + e(8) * z) / 2) * width;
  const sy = (-(e(1) * x + e(5) * y + e(9) * z) / 2) * height;
  // + 0 turns -0 into 0.
  return [sx + 0, sy + 0];
}

/** The hover arrow's size on screen, in pixels (M6): a fifth of the tool
 *  marker's length and half its thickness, so it covers little of the
 *  toolpath; a white dot marks the node it starts from. */
export const TRAVEL_MARKER = { length: 13, shaft: 2.5, headLength: 6, headWidth: 7, dot: 3.5 } as const;

/** What to draw while hovering a node, in screen pixels. */
export type TravelMarkerShape =
  | {
      readonly kind: "arrow";
      readonly dot: { readonly x: number; readonly y: number; readonly r: number };
      /** From the node to the head's base. */
      readonly shaft: readonly [number, number, number, number];
      /** The head: tip, then the two corners of its base. */
      readonly head: readonly [number, number, number, number, number, number];
    }
  | { readonly kind: "dot"; readonly dot: { readonly x: number; readonly y: number; readonly r: number } };

/**
 * The hover arrow at the node (`x`, `y`): (`fx`, `fy`) is how far, in
 * pixels, one mm along the travel moves on screen; `pxPerMm` is the view's
 * scale. Shortened as the travel tilts toward the viewer, and only the dot
 * when it points (nearly) straight at or away from them.
 */
export function travelMarkerShape(x: number, y: number, fx: number, fy: number, pxPerMm: number): TravelMarkerShape {
  const dot = { x, y, r: TRAVEL_MARKER.dot };
  const along = Math.hypot(fx, fy);
  const f = pxPerMm > 0 ? along / pxPerMm : 0;
  if (!(f >= MIN_FORESHORTENING) || !Number.isFinite(f)) return { kind: "dot", dot };
  const ux = fx / along;
  const uy = fy / along;
  const k = Math.min(f, 1);
  const tipX = x + ux * TRAVEL_MARKER.length * k;
  const tipY = y + uy * TRAVEL_MARKER.length * k;
  const bx = tipX - ux * TRAVEL_MARKER.headLength * k;
  const by = tipY - uy * TRAVEL_MARKER.headLength * k;
  const w = TRAVEL_MARKER.headWidth / 2;
  return {
    kind: "arrow",
    dot,
    shaft: [x, y, bx, by],
    head: [tipX, tipY, bx - uy * w, by + ux * w, bx + uy * w, by - ux * w],
  };
}
