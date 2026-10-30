// Standard views and camera fitting, as plain numbers (no three.js), so
// they are tested without a GPU. Machine coordinates: Z is up.

/** A 3D vector. */
export type Vec3 = readonly [number, number, number];

/** A standard view: the direction the camera looks from, and its up. */
export interface StandardView {
  readonly label: string;
  /** Unit vector from the target towards the camera. */
  readonly from: Vec3;
  readonly up: Vec3;
}

const S = 1 / Math.sqrt(3);
/** Top looks down from just off vertical: orbit controls cannot look
 *  exactly along their up axis (Z), and a tiny tilt towards -Y keeps X to
 *  the right and Y up on screen. */
const TILT = 1e-4;
const TOP_Z = Math.sqrt(1 - TILT * TILT);

/** The standard views, by name. Up is always Z (orbit controls fix their
 *  up axis when created). */
export const VIEWS = {
  top: { label: "Top", from: [0, -TILT, TOP_Z], up: [0, 0, 1] },
  front: { label: "Front", from: [0, -1, 0], up: [0, 0, 1] },
  right: { label: "Right", from: [1, 0, 0], up: [0, 0, 1] },
  iso: { label: "Iso", from: [S, -S, S], up: [0, 0, 1] },
} as const satisfies Record<string, StandardView>;

/** A view name. */
export type ViewName = keyof typeof VIEWS;

/** An orthographic camera fitted to a sphere of `radius` around `center`. */
export interface OrthoFit {
  /** Camera position: back along the view direction, outside the sphere. */
  readonly position: Vec3;
  /** Half the visible height, in world units, at zoom 1. */
  readonly halfHeight: number;
  /** Near and far planes, as distances from the camera. */
  readonly near: number;
  readonly far: number;
}

/** Fits an orthographic camera so the sphere fills the view with a margin,
 *  whichever of width and height is narrower. */
export function fitOrtho(center: Vec3, radius: number, view: StandardView, aspect: number): OrthoFit {
  const r = radius > 0 && Number.isFinite(radius) ? radius : 1;
  const a = aspect > 0 && Number.isFinite(aspect) ? aspect : 1;
  const margin = 1.1;
  const halfHeight = a >= 1 ? r * margin : (r * margin) / a;
  const distance = r * 4;
  return {
    position: [
      center[0] + view.from[0] * distance,
      center[1] + view.from[1] * distance,
      center[2] + view.from[2] * distance,
    ],
    halfHeight,
    near: 0,
    far: distance + r * 4,
  };
}

/** The world distance one screen pixel covers for an orthographic camera. */
export function orthoWorldPerPixel(halfHeight: number, zoom: number, heightPx: number): number {
  return (2 * halfHeight) / Math.max(zoom, 1e-12) / Math.max(1, heightPx);
}
