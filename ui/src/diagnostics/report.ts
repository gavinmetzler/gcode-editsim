// The diagnostics report: plain text the owner pastes into a chat or an
// issue (M5.2). Pure, so it is tested; the panel gathers the facts.

import type { Summary } from "./stats";

/** What the app side (Rust) knows about the system. */
export interface SystemInfo {
  readonly os: string;
  readonly arch: string;
  /** The Flatpak runtime (which fixes the WebKitGTK version), or null outside Flatpak. */
  readonly flatpakRuntime: string | null;
  /** The graphics cards the system lists, with their drivers (browsers may
   *  hide the real GPU from WebGL). */
  readonly gpus: readonly string[];
  /** The NVIDIA driver version, when one is loaded. */
  readonly nvidiaDriver: string | null;
  /** Environment variables that change how WebKitGTK draws, as set. */
  readonly env: readonly (readonly [string, string])[];
}

/** Everything the report shows. */
export interface DiagnosticsInfo {
  /** When it was made (ISO 8601). */
  readonly generated: string;
  readonly app: { readonly version: string | null; readonly uiBuild: string };
  /** Null outside the desktop app. */
  readonly system: SystemInfo | null;
  readonly browser: {
    readonly userAgent: string;
    readonly devicePixelRatio: number;
    readonly window: readonly [number, number];
    readonly cpuThreads: number;
  };
  /** Null when WebGL is not available. */
  readonly webgl: {
    readonly version: string;
    readonly shadingLanguage: string;
    /** The real GPU when the browser tells it, else what WebGL reports. */
    readonly renderer: string;
    readonly vendor: string;
    readonly maxTextureSize: number;
    readonly antialias: boolean;
    readonly canvas: readonly [number, number];
    readonly drawingBuffer: readonly [number, number];
    readonly threeRevision: string;
  } | null;
  /** Null before the first run. */
  readonly program: {
    readonly name: string;
    readonly lines: number;
    readonly segments: number;
    readonly vertices: number;
    readonly bufferBytes: number;
    readonly diagnostics: number;
    readonly machine: string;
    readonly dialect: string;
  } | null;
  readonly lastRun: { readonly interpretMs: number; readonly buildMs: number; readonly showMs: number } | null;
  /** three.js's counts for the last frame. */
  readonly gpu: {
    readonly geometries: number;
    readonly textures: number;
    readonly calls: number;
    readonly lines: number;
    readonly points: number;
  } | null;
  readonly timings: readonly { readonly label: string; readonly summary: Summary | null }[];
  /** Frames in which the highlight waited for the editor to catch up. */
  readonly heldBack: number;
  /** The last key held down, or null before the first. */
  readonly lastHold: {
    readonly presses: number;
    readonly keyboardMs: number;
    readonly handledMs: number;
  } | null;
  /** The panel's switches and whether each is on. */
  readonly switches: readonly { readonly label: string; readonly on: boolean }[];
}

const count = (n: number): string => Math.round(n).toLocaleString("en-US");
const ms = (n: number): string => n.toFixed(1);
const size = (s: readonly [number, number]): string => `${String(s[0])} × ${String(s[1])}`;

function timing(label: string, s: Summary | null): string {
  if (!s) return `  ${label}: no samples yet`;
  return (
    `  ${label}: last ${ms(s.last)}, avg ${ms(s.mean)}, 95% ${ms(s.p95)}, max ${ms(s.max)} ms` +
    ` (${count(s.samples)} recent of ${count(s.total)})`
  );
}

/** The report as plain text, one fact per line. */
export function formatReport(d: DiagnosticsInfo): string {
  const out: string[] = [`GCode EditSim diagnostics (${d.generated})`];
  out.push(`App: ${d.app.version ?? "unknown version"} (UI build ${d.app.uiBuild})`);
  if (d.system) {
    const runtime = d.system.flatpakRuntime ? `, Flatpak runtime ${d.system.flatpakRuntime}` : ", not in a Flatpak";
    out.push(`System: ${d.system.os} ${d.system.arch}${runtime}`);
    const cards = d.system.gpus.length === 0 ? "none found" : d.system.gpus.join("; ");
    const nvidia = d.system.nvidiaDriver ? `; NVIDIA driver ${d.system.nvidiaDriver}` : "";
    out.push(`Graphics cards (system): ${cards}${nvidia}`);
    out.push(
      `Environment: ${d.system.env.length === 0 ? "none of the drawing settings set" : d.system.env.map(([k, v]) => `${k}=${v}`).join(", ")}`,
    );
  } else {
    out.push("System: not available (not running in the desktop app)");
  }
  out.push(`Browser: ${d.browser.userAgent}`);
  out.push(
    `Window: ${size(d.browser.window)} CSS px, device pixel ratio ${String(d.browser.devicePixelRatio)}, ${String(d.browser.cpuThreads)} CPU threads`,
  );
  if (d.webgl) {
    const g = d.webgl;
    out.push(`GPU (as WebGL reports it): ${g.renderer} (${g.vendor})`);
    out.push(
      `Graphics: ${g.version}, ${g.shadingLanguage}, max texture ${String(g.maxTextureSize)}, antialias ${g.antialias ? "on" : "off"}, three.js r${g.threeRevision}`,
    );
    out.push(`3D view: ${size(g.canvas)} CSS px, drawing buffer ${size(g.drawingBuffer)} px`);
  } else {
    out.push("GPU: WebGL not available");
  }
  if (d.program) {
    const p = d.program;
    out.push(
      `Program: ${p.name}, ${count(p.lines)} lines, ${count(p.segments)} segments, ${count(p.vertices)} vertices, ` +
        `render buffer ${(p.bufferBytes / 1e6).toFixed(1)} MB, ${count(p.diagnostics)} diagnostics`,
    );
    out.push(`Machine: ${p.machine}; dialect: ${p.dialect}`);
  } else {
    out.push("Program: none run yet");
  }
  if (d.lastRun) {
    const r = d.lastRun;
    out.push(
      `Last run: interpret ${count(r.interpretMs)} ms, build drawing ${count(r.buildMs)} ms, upload and first draw ${count(r.showMs)} ms`,
    );
  }
  if (d.gpu) {
    const g = d.gpu;
    out.push(
      `Last frame: ${count(g.calls)} draw calls, ${count(g.lines)} lines, ${count(g.points)} points; ${count(g.geometries)} geometries, ${count(g.textures)} textures on the GPU`,
    );
  }
  out.push(`Switches: ${d.switches.map((w) => `${w.label} ${w.on ? "on" : "off"}`).join(", ")}`);
  out.push("Timings:");
  for (const t of d.timings) out.push(timing(t.label, t.summary));
  out.push(`Highlight held back while the editor caught up: ${count(d.heldBack)} frames`);
  const h = d.lastHold;
  out.push(
    h
      ? `Last held key: ${count(h.presses)} presses, sent by the keyboard over ${(h.keyboardMs / 1000).toFixed(1)} s, handled by the page over ${(h.handledMs / 1000).toFixed(1)} s`
      : "Last held key: none yet",
  );
  if (h && h.presses > 1 && h.keyboardMs === 0) {
    out.push(
      'Note: the keyboard\'s repeated key events all carry the first press\'s timestamp here, so "Key press waiting" and "Keyboard repeat interval" are not real measurements.',
    );
  }
  return out.join("\n");
}
