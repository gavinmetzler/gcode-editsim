// The Transform dialog's values as the request the app takes (M4.9; the
// request is app_backend::transform::TransformRequest). Values are mm and
// degrees, in program coordinates. Scaling is in X and Y only (owner
// decision: Z is never scaled).

/** Which transformation. */
export type TransformKind = "translate" | "rotate" | "scale";

/** The plane of a rotation, by its axes in arc-plane order. */
export type PlaneName = "XY" | "ZX" | "YZ";

/** The dialog's fields, as typed. */
export interface TransformForm {
  readonly kind: TransformKind;
  readonly dx: string;
  readonly dy: string;
  readonly dz: string;
  readonly plane: PlaneName;
  readonly angle: string;
  readonly factor: string;
  readonly cx: string;
  readonly cy: string;
  readonly cz: string;
}

/** A form with nothing filled in. */
export function emptyForm(): TransformForm {
  return { kind: "translate", dx: "", dy: "", dz: "", plane: "XY", angle: "", factor: "", cx: "", cy: "", cz: "" };
}

type ByAxis = Readonly<Record<"X" | "Y" | "Z", number>>;

/** What the app is sent. */
export interface TransformRequest {
  readonly first: number;
  readonly last: number;
  readonly transform:
    | { readonly kind: "translate"; readonly offset: ByAxis }
    | { readonly kind: "rotate"; readonly plane: readonly [string, string]; readonly angle: number; readonly centre: ByAxis }
    | { readonly kind: "scale"; readonly plane: readonly [string, string]; readonly factor: number; readonly centre: ByAxis };
}

const PLANES: Readonly<Record<PlaneName, readonly [string, string]>> = {
  XY: ["X", "Y"],
  ZX: ["Z", "X"],
  YZ: ["Y", "Z"],
};

/** A plain decimal number (`12`, `-3.5`, `+.5`); an empty field is
 *  `empty`; anything else is null. */
export function parseNumber(text: string, empty: number): number | null {
  const t = text.trim();
  if (t === "") return empty;
  if (!/^[+-]?(\d+\.?\d*|\.\d+)$/.test(t)) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

/** The request for lines `first..last`, or what is wrong with the form. */
export function buildRequest(
  first: number,
  last: number,
  form: TransformForm,
): { readonly ok: true; readonly request: TransformRequest } | { readonly ok: false; readonly error: string } {
  const fail = (error: string) => ({ ok: false, error }) as const;
  const three = (names: readonly [string, string, string], values: readonly [string, string, string]): ByAxis | string => {
    const [x, y, z] = values.map((v) => parseNumber(v, 0));
    if (x === null || x === undefined) return names[0];
    if (y === null || y === undefined) return names[1];
    if (z === null || z === undefined) return names[2];
    return { X: x, Y: y, Z: z };
  };
  const centre = three(["centre X", "centre Y", "centre Z"], [form.cx, form.cy, form.cz]);
  const done = (transform: TransformRequest["transform"]) => ({ ok: true, request: { first, last, transform } }) as const;
  switch (form.kind) {
    case "translate": {
      const offset = three(["dX", "dY", "dZ"], [form.dx, form.dy, form.dz]);
      if (typeof offset === "string") return fail(`${offset} is not a number.`);
      if (offset.X === 0 && offset.Y === 0 && offset.Z === 0) return fail("Give an offset for at least one axis.");
      return done({ kind: "translate", offset });
    }
    case "rotate": {
      const angle = parseNumber(form.angle, Number.NaN);
      if (angle === null || Number.isNaN(angle)) return fail("Give the angle in degrees.");
      if (angle % 360 === 0) return fail("An angle of a whole turn changes nothing.");
      if (typeof centre === "string") return fail(`The ${centre} is not a number.`);
      return done({ kind: "rotate", plane: PLANES[form.plane], angle, centre });
    }
    case "scale": {
      const factor = parseNumber(form.factor, Number.NaN);
      if (factor === null || Number.isNaN(factor) || factor <= 0) {
        return fail("The factor must be a number greater than zero (0.8 is 80 %).");
      }
      if (factor === 1) return fail("A factor of 1 changes nothing.");
      if (typeof centre === "string") return fail(`The ${centre} is not a number.`);
      return done({ kind: "scale", plane: PLANES.XY, factor, centre });
    }
  }
}
