// Rolling timing statistics for the diagnostics panel (M5.2).

/** A summary of the recent samples, in milliseconds. */
export interface Summary {
  /** Samples added since the start (or the last reset). */
  readonly total: number;
  /** Samples the summary covers (the most recent ones). */
  readonly samples: number;
  readonly last: number;
  readonly mean: number;
  /** 95th percentile (nearest rank). */
  readonly p95: number;
  readonly max: number;
}

/** Keeps the most recent `capacity` timings. */
export class RollingStats {
  private readonly values: Float64Array;
  private next = 0;
  private count = 0;
  private total = 0;
  private lastValue = 0;

  constructor(capacity = 120) {
    this.values = new Float64Array(Math.max(1, Math.floor(capacity)));
  }

  /** Adds one timing (ms); values that are not finite or are negative are ignored. */
  add(ms: number): void {
    if (!Number.isFinite(ms) || ms < 0) return;
    this.values[this.next] = ms;
    this.next = (this.next + 1) % this.values.length;
    this.count = Math.min(this.count + 1, this.values.length);
    this.total += 1;
    this.lastValue = ms;
  }

  /** Forgets every sample. */
  reset(): void {
    this.next = 0;
    this.count = 0;
    this.total = 0;
  }

  /** The summary, or null before the first sample. */
  summary(): Summary | null {
    if (this.count === 0) return null;
    const recent = this.values.slice(0, this.count).sort();
    let sum = 0;
    for (const v of recent) sum += v;
    return {
      total: this.total,
      samples: this.count,
      last: this.lastValue,
      mean: sum / this.count,
      p95: recent[Math.ceil(0.95 * this.count) - 1] ?? 0,
      max: recent[this.count - 1] ?? 0,
    };
  }
}

/** The timings the app collects. */
export interface Perf {
  /** How long key presses waited before the page handled them. */
  readonly inputDelay: RollingStats;
  /** Time between a held key's repeats, by the keyboard's timestamps. */
  readonly keyRepeat: RollingStats;
  /** Time the page (the editor) spends handling one key press. */
  readonly keyHandling: RollingStats;
  /** Time between a held key's repeats reaching the page (the page's clock). */
  readonly keyArrival: RollingStats;
  /** Time between the app's own steps for a held cursor key. */
  readonly pacedStep: RollingStats;
  /** Following the cursor: marker, readout, highlight and status. */
  readonly follow: RollingStats;
  /** One 3D draw, as far as JavaScript sees it (the GPU may still be busy). */
  readonly render: RollingStats;
  /** Time between frames, measured while the panel is open. */
  readonly frame: RollingStats;
  /** Frames in which the highlight waited for the editor to catch up. */
  heldBack: number;
}

/** Empty timings. */
export function newPerf(): Perf {
  return {
    inputDelay: new RollingStats(),
    keyRepeat: new RollingStats(),
    keyHandling: new RollingStats(),
    keyArrival: new RollingStats(),
    pacedStep: new RollingStats(),
    follow: new RollingStats(),
    render: new RollingStats(),
    frame: new RollingStats(),
    heldBack: 0,
  };
}
