// The tool marker's shape on screen: a thick shaft with an arrowhead whose
// tip is the point, along the machine's Z axis as the view shows it.
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
 * pixels, one mm along +Z moves on screen; `pxPerMm` is the view's scale.
 * The shaft lies along +Z (up from the tip), shortened as Z tilts toward
 * the viewer.
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
