// The selected lines, drawn bright yellow on top of the toolpath (M4.3),
// by a shader over the geometry already on the graphics card (M5.2).
//
// Every vertex carries its program line (`lineNo`, uploaded once with the
// toolpath). The highlight draws the feed and rapid geometry a second time
// with a shader that keeps only the vertices whose line is in the chosen
// range, and moves the others outside the view, where they are clipped.
// Changing the range sets two numbers: nothing is built, copied or
// uploaded, so highlighting a million lines costs the same as one. This
// is how fast G-code viewers work (PrusaSlicer's libvgcode keeps the
// toolpath on the graphics card and changes only small data when the view
// range moves). Every run of a line is included, subprogram calls too,
// as the line index's `segmentsInLines` lists them.

import {
  BufferAttribute,
  BufferGeometry,
  Color,
  LineSegments,
  Points,
  ShaderMaterial,
} from "three";

/** First and last program line (1-based, inclusive). */
export interface LineRange {
  readonly first: number;
  readonly last: number;
}

/** The attribute name the shader reads each vertex's line from. */
export const LINE_ATTRIBUTE = "lineNo";

const vertexShader = /* glsl */ `
attribute float lineNo;
uniform float hlFirst;
uniform float hlLast;
uniform float hlPointSize;
void main() {
  gl_PointSize = hlPointSize;
  // Line numbers are whole numbers, exact as floats up to 2^24.
  if (lineNo < hlFirst - 0.5 || lineNo > hlLast + 0.5) {
    // Outside the clip volume: the GPU drops it without drawing.
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    return;
  }
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const fragmentShader = /* glsl */ `
uniform vec3 hlColor;
void main() {
  gl_FragColor = vec4(hlColor, 1.0);
  #include <colorspace_fragment>
}
`;

function material(color: Color, pointSize: number): ShaderMaterial {
  return new ShaderMaterial({
    uniforms: {
      hlFirst: { value: 1 },
      hlLast: { value: 0 },
      hlPointSize: { value: pointSize },
      hlColor: { value: color },
    },
    vertexShader,
    fragmentShader,
    depthTest: false,
    depthWrite: false,
  });
}

/** The highlight for one toolpath. */
export class LineHighlight {
  /** What to add to the scene (drawn on top of the toolpath). */
  readonly objects: readonly (LineSegments | Points)[];
  private readonly lineMaterial: ShaderMaterial;
  private readonly pointMaterial: ShaderMaterial;
  private readonly pointGeometry: BufferGeometry;

  /**
   * `lines` are the toolpath's line geometries (their vertices must carry
   * `LINE_ATTRIBUTE`); `points` the single-vertex segments with their
   * lines; `color` in linear light; `pointSize` in device pixels.
   */
  constructor(
    lines: readonly BufferGeometry[],
    points: { readonly positions: Float32Array; readonly lines: Float32Array },
    color: Color,
    pointSize: number,
  ) {
    this.lineMaterial = material(color, 1);
    this.pointMaterial = material(color, pointSize);
    this.pointGeometry = new BufferGeometry();
    this.pointGeometry.setAttribute("position", new BufferAttribute(points.positions, 3));
    this.pointGeometry.setAttribute(LINE_ATTRIBUTE, new BufferAttribute(points.lines, 1));
    const objects: (LineSegments | Points)[] = lines.map((g) => new LineSegments(g, this.lineMaterial));
    objects.push(new Points(this.pointGeometry, this.pointMaterial));
    for (const o of objects) {
      o.renderOrder = 10;
      o.visible = false;
    }
    this.objects = objects;
  }

  /** Highlights the lines from `range.first` to `range.last` (either
   *  order), or nothing for null. */
  set(range: LineRange | null): void {
    const first = range ? Math.min(range.first, range.last) : 1;
    const last = range ? Math.max(range.first, range.last) : 0;
    for (const m of [this.lineMaterial, this.pointMaterial]) {
      const u = m.uniforms;
      if (u.hlFirst) u.hlFirst.value = first;
      if (u.hlLast) u.hlLast.value = last;
    }
    for (const o of this.objects) o.visible = range !== null;
  }

  /** Sets the dots' size in device pixels (after the pixel ratio changes). */
  setPointSize(pointSize: number): void {
    const u = this.pointMaterial.uniforms.hlPointSize;
    if (u) u.value = pointSize;
  }

  /** Releases what this highlight owns (not the shared line geometries). */
  dispose(): void {
    this.lineMaterial.dispose();
    this.pointMaterial.dispose();
    this.pointGeometry.dispose();
  }
}
