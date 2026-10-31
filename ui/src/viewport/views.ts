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

/** The smallest and largest angle the view direction may make with +Z, as
 *  the orbit controls allow (looking straight down or up is not possible
 *  with Z kept up). */
export const MIN_POLAR = 1e-4;
export const MAX_POLAR = Math.PI - 1e-4;

function sub(a: Vec3, b: Vec3): [number, number, number] {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

/** `v` rotated by `angle` about the unit vector `axis` (Rodrigues). */
function rotate(v: Vec3, axis: Vec3, angle: number): [number, number, number] {
  const [x, y, z] = v;
  const [ax, ay, az] = axis;
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  const dot = ax * x + ay * y + az * z;
  const cx = ay * z - az * y;
  const cy = az * x - ax * z;
  const cz = ax * y - ay * x;
  return [x * c + cx * s + ax * dot * (1 - c), y * c + cy * s + ay * dot * (1 - c), z * c + cz * s + az * dot * (1 - c)];
}

/**
 * Orbits the camera about `pivot` instead of about its target: `dTheta`
 * turns about the vertical (+Z) through the pivot, `dPhi` tilts the view
 * toward the horizon (positive) or toward looking down (negative), within
 * MIN_POLAR..MAX_POLAR. The camera and its target turn together as one,
 * so the pivot keeps its place on screen and the view does not jump when
 * the drag starts. Angles are as the orbit controls use them.
 */
export function orbitAbout(
  position: Vec3,
  target: Vec3,
  pivot: Vec3,
  dTheta: number,
  dPhi: number,
): { position: Vec3; target: Vec3 } {
  const o = sub(position, target);
  const r = Math.hypot(o[0], o[1], o[2]);
  let p = sub(position, pivot);
  let t = sub(target, pivot);
  if (r > 0) {
    const phi = Math.acos(Math.min(1, Math.max(-1, o[2] / r)));
    const tilt = Math.min(MAX_POLAR, Math.max(MIN_POLAR, phi + dPhi)) - phi;
    // Tilt about the horizontal axis at right angles to the view.
    const ax = -o[1];
    const ay = o[0];
    const len = Math.hypot(ax, ay);
    if (len > 0 && tilt !== 0) {
      const axis: Vec3 = [ax / len, ay / len, 0];
      p = rotate(p, axis, tilt);
      t = rotate(t, axis, tilt);
    }
  }
  if (dTheta !== 0) {
    p = rotate(p, [0, 0, 1], dTheta);
    t = rotate(t, [0, 0, 1], dTheta);
  }
  return {
    position: [p[0] + pivot[0], p[1] + pivot[1], p[2] + pivot[2]],
    target: [t[0] + pivot[0], t[1] + pivot[1], t[2] + pivot[2]],
  };
}
