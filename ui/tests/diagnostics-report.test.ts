import { describe, expect, it } from "vitest";

import { formatReport, type DiagnosticsInfo } from "../src/diagnostics/report";
import { RollingStats } from "../src/diagnostics/stats";

describe("RollingStats", () => {
  it("summarises the recent samples: last, average, 95th percentile and maximum", () => {
    const s = new RollingStats(100);
    for (let i = 1; i <= 100; i += 1) s.add(i);
    expect(s.summary()).toEqual({ total: 100, samples: 100, last: 100, mean: 50.5, p95: 95, max: 100 });
  });

  it("keeps only the most recent samples, but counts them all", () => {
    const s = new RollingStats(3);
    for (const v of [100, 1, 2, 3]) s.add(v);
    expect(s.summary()).toEqual({ total: 4, samples: 3, last: 3, mean: 2, p95: 3, max: 3 });
  });

  it("has no summary before the first sample, and after a reset", () => {
    const s = new RollingStats();
    expect(s.summary()).toBeNull();
    s.add(5);
    s.reset();
    expect(s.summary()).toBeNull();
  });

  it("ignores values that are not finite or negative", () => {
    const s = new RollingStats();
    s.add(Number.NaN);
    s.add(Number.POSITIVE_INFINITY);
    s.add(-1);
    expect(s.summary()).toBeNull();
  });
});

const full: DiagnosticsInfo = {
  generated: "2026-11-02T10:00:00.000Z",
  app: { version: "0.1.0", uiBuild: "abc1234" },
  system: {
    os: "linux",
    arch: "x86_64",
    flatpakRuntime: "org.gnome.Platform/x86_64/47",
    gpus: ["card1: driver nvidia, PCI 10de:2684"],
    nvidiaDriver: "560.35.03",
    env: [
      ["WEBKIT_DISABLE_DMABUF_RENDERER", "1"],
      ["XDG_SESSION_TYPE", "wayland"],
    ],
  },
  browser: { userAgent: "Mozilla/5.0 (X11) AppleWebKit/605.1.15", devicePixelRatio: 1.5, window: [1920, 1080], cpuThreads: 32 },
  webgl: {
    version: "WebGL 2.0",
    shadingLanguage: "WebGL GLSL ES 3.00",
    renderer: "NVIDIA GeForce RTX 4090/PCIe/SSE2",
    vendor: "NVIDIA Corporation",
    maxTextureSize: 32768,
    antialias: true,
    canvas: [1000, 600],
    drawingBuffer: [1500, 900],
    threeRevision: "186",
  },
  program: {
    name: "big.nc",
    lines: 1_000_012,
    segments: 1_000_013,
    vertices: 4_100_000,
    bufferBytes: 60_800_000,
    diagnostics: 0,
    machine: "Generic 3-axis VMC",
    dialect: "Fanuc (milling)",
  },
  lastRun: { interpretMs: 3684.2, buildMs: 450.4, showMs: 120.9 },
  gpu: { geometries: 4, textures: 0, calls: 5, lines: 2_000_000, points: 10 },
  timings: [
    { label: "Key press waiting", summary: { total: 534, samples: 120, last: 3.24, mean: 2.1, p95: 4, max: 9.94 } },
    { label: "3D draw (JavaScript)", summary: null },
  ],
  heldBack: 12,
  lastHold: { presses: 90, keyboardMs: 3000, handledMs: 25_000 },
};

describe("formatReport", () => {
  const text = formatReport(full);

  it("starts with a title and the time it was made", () => {
    expect(text.split("\n")[0]).toBe("GCode EditSim diagnostics (2026-11-02T10:00:00.000Z)");
  });

  it("names the app, system, Flatpak runtime and the environment that affects drawing", () => {
    expect(text).toContain("App: 0.1.0 (UI build abc1234)");
    expect(text).toContain("System: linux x86_64, Flatpak runtime org.gnome.Platform/x86_64/47");
    expect(text).toContain("WEBKIT_DISABLE_DMABUF_RENDERER=1");
    expect(text).toContain("XDG_SESSION_TYPE=wayland");
  });

  it("names the graphics cards and driver the system reports, since browsers may hide the GPU", () => {
    expect(text).toContain("Graphics cards (system): card1: driver nvidia, PCI 10de:2684; NVIDIA driver 560.35.03");
    if (!full.system) throw new Error("the sample has system facts");
    const none = formatReport({ ...full, system: { ...full.system, gpus: [], nvidiaDriver: null } });
    expect(none).toContain("Graphics cards (system): none found");
  });

  it("compares the keyboard's time for the last held key with the page's", () => {
    expect(text).toContain("Last held key: 90 presses, sent by the keyboard over 3.0 s, handled by the page over 25.0 s");
    expect(formatReport({ ...full, lastHold: null })).toContain("Last held key: none yet");
  });

  it("counts the frames the highlight waited for the editor to catch up", () => {
    expect(text).toContain("Highlight held back while the editor caught up: 12 frames");
  });

  it("describes the graphics: WebGL version, GPU, sizes", () => {
    expect(text).toContain("GPU (as WebGL reports it): NVIDIA GeForce RTX 4090/PCIe/SSE2 (NVIDIA Corporation)");
    expect(text).toContain("WebGL 2.0, WebGL GLSL ES 3.00, max texture 32768, antialias on, three.js r186");
    expect(text).toContain("3D view: 1000 × 600 CSS px, drawing buffer 1500 × 900 px");
    expect(text).toContain("Window: 1920 × 1080 CSS px, device pixel ratio 1.5, 32 CPU threads");
  });

  it("gives the program's size with thousands separators, and the last run's times", () => {
    expect(text).toContain("Program: big.nc, 1,000,012 lines, 1,000,013 segments, 4,100,000 vertices");
    expect(text).toContain("render buffer 60.8 MB, 0 diagnostics");
    expect(text).toContain("Machine: Generic 3-axis VMC; dialect: Fanuc (milling)");
    expect(text).toContain("Last run: interpret 3,684 ms, build drawing 450 ms, upload and first draw 121 ms");
  });

  it("lists each timing as last / average / 95% / maximum, or that there are none yet", () => {
    expect(text).toContain("Key press waiting: last 3.2, avg 2.1, 95% 4.0, max 9.9 ms (120 recent of 534)");
    expect(text).toContain("3D draw (JavaScript): no samples yet");
  });

  it("says what is not available instead of leaving it out", () => {
    const sparse = formatReport({ ...full, app: { version: null, uiBuild: "unknown" }, system: null, webgl: null, program: null, lastRun: null, gpu: null });
    expect(sparse).toContain("App: unknown version (UI build unknown)");
    expect(sparse).toContain("System: not available (not running in the desktop app)");
    expect(sparse).toContain("GPU: WebGL not available");
    expect(sparse).toContain("Program: none run yet");
  });
});
