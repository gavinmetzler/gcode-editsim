// Held cursor keys, paced by the app (M5.2). Every key event the page
// receives is handled in turn, and on the owner's machine (WebKitGTK,
// Wayland, frames copied through the CPU) each one cost about 235 ms to
// get on screen while the keyboard sent about 30 a second, so a held
// Page Down piled up seconds of presses that kept scrolling after the
// key was released. Frames, unlike key events, are merged: however often
// the text changes between two frames, one frame is drawn.
//
// So the first press of ↑ ↓ Page Up or Page Down (with or without Shift)
// goes to the editor as usual, but once the keyboard repeats it, the
// repeats are ignored (cheap: they change nothing on screen) and the app
// moves the cursor itself, at most `PACE.minInterval` apart, until the key
// is released. Releasing the key stops it at once.

/** The keys paced while held. */
export type PacedKey = "ArrowUp" | "ArrowDown" | "PageUp" | "PageDown";

const PACED: ReadonlySet<string> = new Set(["ArrowUp", "ArrowDown", "PageUp", "PageDown"]);

/** The modifier keys of a key event. */
export interface Modifiers {
  readonly shift: boolean;
  readonly ctrl: boolean;
  readonly alt: boolean;
  readonly meta: boolean;
}

/** One step: move the cursor as `key` would, extending the selection when
 *  `extend` (Shift). */
export interface Step {
  readonly key: PacedKey;
  readonly extend: boolean;
}

/** The pace, in milliseconds: about 30 steps a second, like a keyboard;
 *  every second frame at 60 Hz, with room for frame-time jitter. */
export const PACE = { minInterval: 30 } as const;

/** See the module comment. Pure: the caller passes the events and the
 *  time, and runs the steps. */
export class PacedKeys {
  private on = true;
  private held: { key: PacedKey; extend: boolean } | null = null;
  private pacing = false;
  private lastStep = Number.NEGATIVE_INFINITY;

  /** Whether held keys are paced (a switch in the diagnostics panel). */
  get enabled(): boolean {
    return this.on;
  }

  set enabled(on: boolean) {
    this.on = on;
    if (!on) this.stop();
  }

  /** True while a held key is being paced. */
  get active(): boolean {
    return this.pacing && this.held !== null;
  }

  /** A key went down (or repeated). Returns true when the app takes it, so
   *  the editor must ignore it. */
  keyDown(key: string, mods: Modifiers, repeat: boolean): boolean {
    if (!PACED.has(key) || mods.ctrl || mods.alt || mods.meta) {
      this.stop();
      return false;
    }
    const k = key as PacedKey;
    if (!repeat) {
      // A fresh press: the editor moves the cursor; pacing starts with the
      // keyboard's first repeat, after its own repeat delay.
      this.stop();
      this.held = { key: k, extend: mods.shift };
      return false;
    }
    if (!this.on) return false;
    if (this.held?.key !== k) this.lastStep = Number.NEGATIVE_INFINITY;
    this.held = { key: k, extend: mods.shift };
    this.pacing = true;
    return true;
  }

  /** A key was released. */
  keyUp(key: string): void {
    if (this.held?.key === key) this.stop();
  }

  /** Ends any hold. */
  stop(): void {
    this.held = null;
    this.pacing = false;
    this.lastStep = Number.NEGATIVE_INFINITY;
  }

  /** Called every frame: the step to run now, or null. */
  tick(now: number): Step | null {
    if (!this.on || !this.pacing || !this.held) return null;
    if (now - this.lastStep < PACE.minInterval) return null;
    this.lastStep = now;
    return { key: this.held.key, extend: this.held.extend };
  }
}
