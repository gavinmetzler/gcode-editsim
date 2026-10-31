// The 3D toolpath view (three.js). Draws what buildToolpath produced, with
// orbit/pan/zoom, standard views, fit to the toolpath, picking, hovering
// and the tool marker.
//
// Vertex positions are relative to the buffer's origin (ARCHITECTURE.md
// section 8.2); the origin is applied on the object's transform, which
// three.js composes in double precision before the GPU sees it.

import {
  AxesHelper,
  BufferAttribute,
  BufferGeometry,
  Color,
  Group,
  LineBasicMaterial,
  LineDashedMaterial,
  LineSegments,
  Matrix4,
  OrthographicCamera,
  Points,
  PointsMaterial,
  Scene,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

import type { ToolPoint } from "./readout";
import { MARKER, markerShape } from "./marker";
import { blockMiddleClickPaste } from "./middle-click";
import type { Measurement } from "./measure";
import { nearestScreenPoint } from "./snap";
import { expandIndexed, type Toolpath } from "./toolpath";
import { VIEWS, fitOrtho, orbitAbout, orthoWorldPerPixel, type Vec3, type ViewName } from "./views";


const SVG = "http://www.w3.org/2000/svg";
/** The marker's colour. */
const MARKER_COLOR = "#ffd23f";

/** A toolpath viewport inside `container`. */
export class Viewport {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  // Orthographic, as backplots usually are: no perspective, so heights do
  // not shift positions in the standard views.
  private readonly camera = new OrthographicCamera(-1, 1, 1, -1, 0, 1);
  /** Half the visible height at zoom 1, set by the last fit. */
  private halfHeight = 10;
  private readonly controls: OrbitControls;
  private readonly group = new Group();
  /** The move end points on screen; recomputed after the camera moves. */
  private screen: Float32Array | null = null;
  private toolpath: Toolpath | null = null;
  private feed: LineSegments | null = null;
  private rapid: LineSegments | null = null;
  private markers: Points | null = null;
  /** GPU resources of what is shown, released by clear(). */
  private disposables: { dispose(): void }[] = [];
  private axes: AxesHelper | null = null;
  private highlighted: { objects: (LineSegments | Points)[]; resources: { dispose(): void }[] } | null = null;
  private view: ViewName = "iso";
  private readonly resizeObserver: ResizeObserver;
  private frame = 0;
  private pointerDown: { x: number; y: number } | null = null;
  /** The flat overlay the tool marker is drawn on (see marker.ts). */
  private readonly overlay = document.createElementNS(SVG, "svg");
  /** Where the marker's tip is (machine coordinates), or null. */
  private markerPoint: Vec3 | null = null;
  /** Measure mode, the measurement shown, whether a measuring drag is in
   *  progress, and the node the pointer is snapped to (measure mode). */
  private measuring = false;
  private measurement: Measurement | null = null;
  private dragging = false;
  private snapped: Vec3 | null = null;
  /** An orbit about a node, while the left button is down. */
  private pivot: { point: Vec3; x: number; y: number; startX: number; startY: number } | null = null;
  private hoverFrame = 0;

  /** Called with the toolpath point a click picked (snapped as for
   *  hovering), or null for empty space. */
  onPick: (point: ToolPoint | null) => void = () => undefined;

  /** Called as the pointer moves over the toolpath (snapped to an end point
   *  when near one), and with null when it is over nothing. Not called in
   *  measure mode. */
  onHover: (point: ToolPoint | null) => void = () => undefined;

  /** Called as a measurement changes (null when it is cleared). */
  onMeasure: (measurement: Measurement | null) => void = () => undefined;

  constructor(private readonly container: HTMLElement) {
    this.renderer = new WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setClearColor(new Color(0x14171c));
    container.appendChild(this.renderer.domElement);
    this.camera.up.set(0, 0, 1);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.addEventListener("change", () => {
      this.screen = null;
      this.requestRender();
    });
    this.scene.add(this.group);
    this.overlay.setAttribute("class", "marker-overlay");
    container.appendChild(this.overlay);

    const canvas = this.renderer.domElement;
    // The middle button zooms here; it must not paste the Linux primary
    // selection into the editor.
    blockMiddleClickPaste(canvas);
    // In measure mode, a left press on a node starts a measurement. This
    // listens on the container in the capture phase, so it runs before the
    // orbit controls (on the canvas) and can keep them from orbiting.
    container.addEventListener(
      "pointerdown",
      (e) => {
        if (e.button !== 0 || e.target !== canvas) return;
        // Outside measure mode, a left (rotate) press on a node orbits about
        // that node. Modifier keys make the left button pan: leave those,
        // and touch, to the orbit controls.
        if (!this.measuring) {
          if (e.shiftKey || e.ctrlKey || e.metaKey || e.pointerType === "touch") return;
          const node = this.pointAt(e.clientX, e.clientY);
          if (!node) return; // No node snapped: orbit as usual.
          e.stopPropagation();
          e.preventDefault();
          canvas.setPointerCapture(e.pointerId);
          this.pivot = { point: node.point, x: e.clientX, y: e.clientY, startX: e.clientX, startY: e.clientY };
          return;
        }
        const p = this.pointAt(e.clientX, e.clientY);
        if (!p) return; // Not on a node: orbit as usual.
        e.stopPropagation();
        e.preventDefault();
        canvas.setPointerCapture(e.pointerId);
        this.dragging = true;
        this.measurement = { start: p.point, end: null };
        this.snapped = p.point;
        this.drawOverlay();
        this.onMeasure(this.measurement);
      },
      { capture: true },
    );
    canvas.addEventListener("pointerdown", (e) => {
      this.pointerDown = { x: e.clientX, y: e.clientY };
    });
    canvas.addEventListener("pointerup", (e) => {
      const pivot = this.pivot;
      if (pivot) {
        this.pivot = null;
        // Hardly moved: a click, which picks as usual.
        if (Math.hypot(e.clientX - pivot.startX, e.clientY - pivot.startY) < 4) {
          this.onPick(this.pointAt(e.clientX, e.clientY));
        }
        return;
      }
      if (this.measuring) {
        this.pointerDown = null;
        if (this.dragging) {
          this.dragging = false;
          // Released without reaching a second node: no measurement.
          if (!this.measurement?.end) this.measurement = null;
          this.drawOverlay();
          this.onMeasure(this.measurement);
        }
        return;
      }
      const down = this.pointerDown;
      this.pointerDown = null;
      // A drag orbits; only a click (little movement) picks.
      if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) < 4) {
        this.onPick(this.pointAt(e.clientX, e.clientY));
      }
    });
    canvas.addEventListener("pointermove", (e) => {
      if (this.pivot) {
        this.orbitAboutPivot(e.clientX, e.clientY);
        return;
      }
      // While a button is down the view is orbiting or panning, unless a
      // measuring drag is in progress.
      if (e.buttons !== 0 && !this.dragging) return;
      const { clientX, clientY } = e;
      cancelAnimationFrame(this.hoverFrame);
      this.hoverFrame = requestAnimationFrame(() => {
        const p = this.pointAt(clientX, clientY);
        if (!this.measuring) {
          this.onHover(p);
          return;
        }
        this.snapped = p?.point ?? null;
        const ms = this.measurement;
        // The end follows the last node reached that is not the start.
        if (this.dragging && ms && p && !p.point.every((c, k) => c === ms.start[k])) {
          this.measurement = { start: ms.start, end: p.point };
          this.onMeasure(this.measurement);
        }
        this.drawOverlay();
      });
    });
    canvas.addEventListener("pointerleave", () => {
      cancelAnimationFrame(this.hoverFrame);
      if (this.measuring) {
        this.snapped = null;
        this.drawOverlay();
        return;
      }
      this.onHover(null);
    });
    this.resizeObserver = new ResizeObserver(() => {
      this.resize();
    });
    this.resizeObserver.observe(container);
    this.resize();
  }

  /** Shows `toolpath`, replacing what was shown. With `fit` (a new
   *  program) the view is fitted to it; without (an edit) the camera stays. */
  show(toolpath: Toolpath, fit = true): void {
    this.clear();
    this.toolpath = toolpath;
    const [ox, oy, oz] = toolpath.origin;
    this.group.position.set(ox, oy, oz);

    const position = new BufferAttribute(toolpath.positions, 3);
    const color = new BufferAttribute(toolpath.colors, 3);
    const lines = (index: Uint32Array): BufferGeometry => {
      const g = new BufferGeometry();
      g.setAttribute("position", position);
      g.setAttribute("color", color);
      g.setIndex(new BufferAttribute(index, 1));
      return g;
    };
    const feedMaterial = new LineBasicMaterial({ vertexColors: true });
    const b = toolpath.bounds;
    const size = b ? Math.max(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2], 1e-3) : 10;
    // Dashes scale with the toolpath, so they stay visible at any size.
    const rapidMaterial = new LineDashedMaterial({
      vertexColors: true,
      dashSize: size / 120,
      gapSize: size / 160,
      transparent: true,
      opacity: 0.8,
    });
    const markerMaterial = new PointsMaterial({ vertexColors: true, size: 7, sizeAttenuation: false });
    this.feed = new LineSegments(lines(toolpath.feedIndex), feedMaterial);
    // Rapids are non-indexed: dash distances need it. Vertex k is
    // rapidIndex[k], so picking maps back the same way as for indexed lines.
    const rapidGeometry = new BufferGeometry();
    rapidGeometry.setAttribute(
      "position",
      new BufferAttribute(expandIndexed(toolpath.positions, toolpath.rapidIndex, 3), 3),
    );
    rapidGeometry.setAttribute("color", new BufferAttribute(expandIndexed(toolpath.colors, toolpath.rapidIndex, 3), 3));
    this.rapid = new LineSegments(rapidGeometry, rapidMaterial);
    this.rapid.computeLineDistances();
    const markerGeometry = new BufferGeometry();
    markerGeometry.setAttribute("position", new BufferAttribute(toolpath.eventPositions, 3));
    markerGeometry.setAttribute("color", new BufferAttribute(toolpath.eventColors, 3));
    this.markers = new Points(markerGeometry, markerMaterial);
    this.group.add(this.feed, this.rapid, this.markers);
    this.disposables.push(
      this.feed.geometry,
      feedMaterial,
      this.rapid.geometry,
      rapidMaterial,
      markerGeometry,
      markerMaterial,
    );

    // Axes at machine zero (X red, Y green, Z blue), not at the buffer's
    // internal origin.
    const axes = new AxesHelper(Math.max(size * 0.08, 1));
    this.scene.add(axes);
    this.axes = axes;
    this.disposables.push(axes);
    if (fit) this.fit();
    else this.requestRender();
  }

  /** Highlights `segments` (indices) on top of the toolpath, replacing any
   *  earlier highlight; an empty list clears it. */
  highlight(segments: readonly number[]): void {
    this.clearHighlight();
    const toolpath = this.toolpath;
    if (!toolpath || segments.length === 0) {
      this.requestRender();
      return;
    }
    const pairs: number[] = [];
    const points: number[] = [];
    for (const s of segments) {
      const range = toolpath.segmentVertices(s);
      if (!range) continue;
      if (range.count === 1) {
        points.push(...toolpath.positions.subarray(range.first * 3, range.first * 3 + 3));
      }
      for (let v = range.first; v + 1 < range.first + range.count; v += 1) pairs.push(v, v + 1);
    }
    const lineGeometry = new BufferGeometry();
    lineGeometry.setAttribute("position", new BufferAttribute(toolpath.positions, 3));
    lineGeometry.setIndex(new BufferAttribute(Uint32Array.from(pairs), 1));
    const lineMaterial = new LineBasicMaterial({ color: 0xffffff, depthTest: false });
    const pointGeometry = new BufferGeometry();
    pointGeometry.setAttribute("position", new BufferAttribute(Float32Array.from(points), 3));
    const pointMaterial = new PointsMaterial({ color: 0xffffff, size: 11, sizeAttenuation: false, depthTest: false });
    const lines = new LineSegments(lineGeometry, lineMaterial);
    const marks = new Points(pointGeometry, pointMaterial);
    lines.renderOrder = 10;
    marks.renderOrder = 10;
    this.group.add(lines, marks);
    this.highlighted = { objects: [lines, marks], resources: [lineGeometry, lineMaterial, pointGeometry, pointMaterial] };
    this.requestRender();
  }

  private clearHighlight(): void {
    if (!this.highlighted) return;
    for (const o of this.highlighted.objects) this.group.remove(o);
    for (const r of this.highlighted.resources) r.dispose();
    this.highlighted = null;
  }

  /** Points the camera at the toolpath from the current standard view. */
  fit(): void {
    this.setView(this.view);
  }

  /** Looks at the toolpath from a standard view. */
  setView(name: ViewName): void {
    this.view = name;
    const view = VIEWS[name];
    const b = this.toolpath?.bounds;
    const [ox, oy, oz] = this.toolpath?.origin ?? [0, 0, 0];
    const min = b?.min ?? [-5, -5, -5];
    const max = b?.max ?? [5, 5, 5];
    const center: [number, number, number] = [
      ox + (min[0] + max[0]) / 2,
      oy + (min[1] + max[1]) / 2,
      oz + (min[2] + max[2]) / 2,
    ];
    const radius = Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]) / 2;
    const rect = this.container.getBoundingClientRect();
    const fit = fitOrtho(center, radius, view, rect.width / Math.max(1, rect.height));
    this.camera.position.set(fit.position[0], fit.position[1], fit.position[2]);
    // Up stays Z: orbit controls fix their up axis when they are created.
    this.halfHeight = fit.halfHeight;
    this.camera.zoom = 1;
    this.camera.near = fit.near;
    this.camera.far = fit.far;
    this.updateFrustum();
    this.controls.target.set(...center);
    this.controls.update();
    this.requestRender();
  }

  /**
   * The move end point nearest the screen point (client coordinates), if one
   * is within a few pixels; null otherwise. Only move end points snap (not
   * points along a move, nor events), which keeps hovering quick on large
   * programs: the end points are projected to the screen once per camera
   * change, then each pointer position is a simple search.
   */
  pointAt(clientX: number, clientY: number): ToolPoint | null {
    const toolpath = this.toolpath;
    if (!toolpath) return null;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.screen ??= this.projectEndpoints(rect.width, rect.height);
    const i = nearestScreenPoint(this.screen, clientX - rect.left, clientY - rect.top);
    const segment = toolpath.endpoints.segment[i];
    if (i < 0 || segment === undefined) return null;
    const [ox, oy, oz] = toolpath.origin;
    const p = toolpath.endpoints.positions;
    return {
      point: [(p[i * 3] ?? NaN) + ox, (p[i * 3 + 1] ?? NaN) + oy, (p[i * 3 + 2] ?? NaN) + oz],
      segment,
      place: toolpath.endpoints.isStart[i] === 1 ? "start" : "end",
    };
  }

  /** Shows the tool marker with its tip at `point` (machine coordinates),
   *  or hides it for null. */
  setMarker(point: Vec3 | null): void {
    const next = point && point.every((c) => Number.isFinite(c)) ? point : null;
    const same =
      next === this.markerPoint ||
      (next !== null && this.markerPoint !== null && next.every((c, k) => c === this.markerPoint?.[k]));
    if (same) return;
    this.markerPoint = next;
    // Only the overlay changes: the toolpath is not redrawn.
    this.drawOverlay();
  }

  /** Turns measure mode on or off. In it, a left drag from one move end
   *  point to another measures between them; a drag starting elsewhere
   *  still orbits. The tool marker is hidden, a ring shows the snapped
   *  node, and clicks do not pick. Turning it off clears the measurement. */
  setMeasureMode(on: boolean): void {
    this.measuring = on;
    this.measurement = null;
    this.dragging = false;
    this.snapped = null;
    this.drawOverlay();
    this.onMeasure(null);
  }

  /** Turns the view about the pivot node for a pointer move to (`x`, `y`),
   *  at the orbit controls' speed (a full turn per view height). */
  private orbitAboutPivot(x: number, y: number): void {
    const pivot = this.pivot;
    if (!pivot) return;
    const height = Math.max(1, this.renderer.domElement.clientHeight);
    const speed = (2 * Math.PI * this.controls.rotateSpeed) / height;
    const dTheta = -(x - pivot.x) * speed;
    const dPhi = -(y - pivot.y) * speed;
    pivot.x = x;
    pivot.y = y;
    const c = this.camera.position;
    const t = this.controls.target;
    const next = orbitAbout([c.x, c.y, c.z], [t.x, t.y, t.z], pivot.point, dTheta, dPhi);
    this.camera.position.set(...next.position);
    this.controls.target.set(...next.target);
    // Looks at the moved target; fires "change", which redraws.
    this.controls.update();
  }

  /** Whether measure mode is on. */
  get measureMode(): boolean {
    return this.measuring;
  }

  /** Draws the overlay for the current camera: the tool marker, or in
   *  measure mode the measurement line and the snapped node. */
  private drawOverlay(): void {
    this.camera.updateMatrixWorld();
    const m = new Matrix4().multiplyMatrices(this.camera.projectionMatrix, this.camera.matrixWorldInverse).elements;
    const width = Math.max(1, this.container.clientWidth);
    const height = Math.max(1, this.container.clientHeight);
    const e = (k: number): number => m[k] ?? 0;
    const screen = (p: Vec3): [number, number] => {
      const [x, y, z] = p;
      return [
        ((e(0) * x + e(4) * y + e(8) * z + e(12) + 1) / 2) * width,
        ((1 - (e(1) * x + e(5) * y + e(9) * z + e(13))) / 2) * height,
      ];
    };
    const el = (tag: string, attrs: Record<string, string | number>): SVGElement => {
      const node = document.createElementNS(SVG, tag);
      for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
      return node;
    };

    if (this.measuring) {
      const parts: SVGElement[] = [];
      const ms = this.measurement;
      if (ms) {
        const [ax, ay] = screen(ms.start);
        if (ms.end) {
          const [bx, by] = screen(ms.end);
          parts.push(
            el("line", { x1: ax, y1: ay, x2: bx, y2: by, stroke: MARKER_COLOR, "stroke-width": 3 }),
            el("circle", { cx: bx, cy: by, r: 4, fill: MARKER_COLOR }),
          );
        }
        parts.push(el("circle", { cx: ax, cy: ay, r: 4, fill: MARKER_COLOR }));
      }
      if (this.snapped) {
        const [sx, sy] = screen(this.snapped);
        parts.push(el("circle", { cx: sx, cy: sy, r: 8, fill: "none", stroke: "#ffffff", "stroke-width": 2 }));
      }
      this.overlay.replaceChildren(...parts);
      return;
    }

    const p = this.markerPoint;
    if (!p) {
      this.overlay.replaceChildren();
      return;
    }
    const [tx, ty] = screen(p);
    // One mm along +Z, on screen. Linear (orthographic), so read off the
    // matrix rather than projecting a second point far from the origin.
    const zx = (e(8) / 2) * width;
    const zy = (-e(9) / 2) * height;
    const pxPerMm = 1 / orthoWorldPerPixel(this.halfHeight, this.camera.zoom, height);
    const shape = markerShape(tx, ty, zx, zy, pxPerMm);
    if (shape.kind === "ring") {
      this.overlay.replaceChildren(
        el("circle", { cx: shape.x, cy: shape.y, r: shape.r, fill: "none", stroke: MARKER_COLOR, "stroke-width": 3 }),
        el("circle", { cx: shape.x, cy: shape.y, r: 2, fill: MARKER_COLOR }),
      );
      return;
    }
    const [x1, y1, x2, y2] = shape.shaft;
    this.overlay.replaceChildren(
      el("line", { x1, y1, x2, y2, stroke: MARKER_COLOR, "stroke-width": MARKER.shaft, "stroke-linecap": "round" }),
      el("polygon", { points: shape.head.join(" "), fill: MARKER_COLOR }),
    );
  }

  /** The end points on screen, as x, y pairs in pixels relative to the
   *  canvas (`width` by `height`), NaN for points not on screen. Computed
   *  in double precision from the camera's matrices. */
  private projectEndpoints(width: number, height: number): Float32Array {
    const toolpath = this.toolpath;
    if (!toolpath) return new Float32Array();
    this.camera.updateMatrixWorld();
    const m = new Matrix4().multiplyMatrices(this.camera.projectionMatrix, this.camera.matrixWorldInverse).elements;
    const [ox, oy, oz] = toolpath.origin;
    const p = toolpath.endpoints.positions;
    const n = p.length / 3;
    const out = new Float32Array(n * 2);
    const e = (k: number): number => m[k] ?? 0;
    for (let i = 0; i < n; i += 1) {
      const x = (p[i * 3] ?? NaN) + ox;
      const y = (p[i * 3 + 1] ?? NaN) + oy;
      const z = (p[i * 3 + 2] ?? NaN) + oz;
      // Orthographic: w is 1, so clip coordinates are normalised already.
      const nx = e(0) * x + e(4) * y + e(8) * z + e(12);
      const ny = e(1) * x + e(5) * y + e(9) * z + e(13);
      out[i * 2] = ((nx + 1) / 2) * width;
      out[i * 2 + 1] = ((1 - ny) / 2) * height;
    }
    return out;
  }

  /** Releases the GPU resources. */
  dispose(): void {
    this.clear();
    this.resizeObserver.disconnect();
    cancelAnimationFrame(this.hoverFrame);
    this.controls.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  private clear(): void {
    this.clearHighlight();
    this.group.clear();
    if (this.axes) this.scene.remove(this.axes);
    this.axes = null;
    for (const d of this.disposables) d.dispose();
    this.disposables = [];
    this.feed = null;
    this.rapid = null;
    this.markers = null;
    this.toolpath = null;
    this.screen = null;
    // A new toolpath has new nodes: an old measurement may not match it.
    if (this.measurement || this.snapped) {
      this.measurement = null;
      this.snapped = null;
      this.dragging = false;
      this.onMeasure(null);
    }
  }

  private resize(): void {
    const width = Math.max(1, this.container.clientWidth);
    const height = Math.max(1, this.container.clientHeight);
    this.renderer.setSize(width, height);
    this.updateFrustum();
    this.requestRender();
  }

  /** Sets the camera's frustum from `halfHeight` and the canvas shape. */
  private updateFrustum(): void {
    this.screen = null;
    const width = Math.max(1, this.container.clientWidth);
    const height = Math.max(1, this.container.clientHeight);
    const halfWidth = (this.halfHeight * width) / height;
    this.camera.left = -halfWidth;
    this.camera.right = halfWidth;
    this.camera.top = this.halfHeight;
    this.camera.bottom = -this.halfHeight;
    this.camera.updateProjectionMatrix();
  }

  private requestRender(): void {
    if (this.frame !== 0) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.renderer.render(this.scene, this.camera);
      // The camera may have moved: put the marker where the point now is.
      this.drawOverlay();
    });
  }
}
