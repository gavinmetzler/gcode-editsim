// The 3D toolpath view (three.js). Draws what buildToolpath produced, with
// orbit/pan/zoom, standard views, fit to the toolpath, and picking.
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
  OrthographicCamera,
  Points,
  PointsMaterial,
  Raycaster,
  Scene,
  Vector2,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

import type { Toolpath } from "./toolpath";
import { VIEWS, fitOrtho, orthoWorldPerPixel, type ViewName } from "./views";

/** How close (in screen pixels) a click must be to pick a line or marker. */
const PICK_PIXELS = 6;

/** What a click picked. */
export interface Pick {
  /** Segment index in the render buffer. */
  readonly segment: number;
}

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
  private readonly raycaster = new Raycaster();
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

  /** Called with what a click picked, or null for empty space. */
  onPick: (pick: Pick | null) => void = () => undefined;

  constructor(private readonly container: HTMLElement) {
    this.renderer = new WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setClearColor(new Color(0x14171c));
    container.appendChild(this.renderer.domElement);
    this.camera.up.set(0, 0, 1);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.addEventListener("change", () => {
      this.requestRender();
    });
    this.scene.add(this.group);

    const canvas = this.renderer.domElement;
    canvas.addEventListener("pointerdown", (e) => {
      this.pointerDown = { x: e.clientX, y: e.clientY };
    });
    canvas.addEventListener("pointerup", (e) => {
      const down = this.pointerDown;
      this.pointerDown = null;
      // A drag orbits; only a click (little movement) picks.
      if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) < 4) {
        this.onPick(this.pick(e.clientX, e.clientY));
      }
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
    this.rapid = new LineSegments(lines(toolpath.rapidIndex), rapidMaterial);
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

  /** What is under the screen point (client coordinates), if anything. */
  pick(clientX: number, clientY: number): Pick | null {
    const toolpath = this.toolpath;
    if (!toolpath) return null;
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      -((clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(ndc, this.camera);
    const tolerance = orthoWorldPerPixel(this.halfHeight, this.camera.zoom, rect.height) * PICK_PIXELS;
    this.raycaster.params.Line.threshold = tolerance;
    this.raycaster.params.Points.threshold = tolerance;
    const targets = [this.markers, this.feed, this.rapid].filter((o) => o !== null);
    const hit = this.raycaster.intersectObjects(targets, false)[0];
    if (!hit || hit.index === undefined) return null;
    if (hit.object === this.markers) {
      const segment = toolpath.eventSegment[hit.index];
      return segment === undefined ? null : { segment };
    }
    // For indexed lines, `index` is the position in the index list.
    const lines = hit.object === this.feed ? toolpath.feedIndex : toolpath.rapidIndex;
    const vertex = lines[hit.index];
    const segment = vertex === undefined ? undefined : toolpath.vertexSegment[vertex];
    return segment === undefined ? null : { segment };
  }

  /** Releases the GPU resources. */
  dispose(): void {
    this.clear();
    this.resizeObserver.disconnect();
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
    });
  }
}
