// Pane sizes and maximising: pure functions over numbers and names, so
// they are tested without a DOM. layout/splitters.ts wires them to the
// page.

import type { SeverityName } from "../panels/diagnostics";

/** The panes the user can resize and maximise. */
export type PaneName = "editor" | "viewport" | "panel";

/** The smallest a pane may be dragged to, in pixels. */
export const MIN_PANE_PIXELS = 120;

/**
 * The share (0 to 1) of a `length`-pixel span that the first pane gets
 * when the splitter is dragged to `offset` pixels from the span's start.
 * Both panes keep at least `min` pixels; a span too small for that is
 * split in half. Never NaN, whatever the input.
 */
export function splitFraction(offset: number, length: number, min = MIN_PANE_PIXELS): number {
  if (!Number.isFinite(length) || length <= 0) return 0.5;
  if (length < 2 * min) return 0.5;
  const at = Number.isFinite(offset) ? offset : length / 2;
  const clamped = Math.min(Math.max(at, min), length - min);
  return clamped / length;
}

/** The maximised pane after a toggle on `pane`: maximises it, or restores
 *  the normal layout if it is the one already maximised. */
export function toggleMaximised(current: PaneName | null, pane: PaneName): PaneName | null {
  return current === pane ? null : pane;
}

/** The panes' layout as remembered between sessions (M4.2). Missing
 *  values are at their defaults. */
export interface PaneLayout {
  /** The editor's share of the window's width. */
  readonly editor?: number;
  /** The diagnostics panel's share of the right-hand column's height. */
  readonly panel?: number;
  /** The maximised pane. */
  readonly maximised?: string;
  /** The colour key is folded (M4.2a). */
  readonly legend_folded?: boolean;
  /** The diagnostics severities whose filter is unticked (M4.2a). */
  readonly hidden_diagnostics?: readonly string[];
}

/**
 * A remembered share applied to a `length`-pixel span now: kept, or
 * clamped so both panes keep at least `min` pixels (the window may be
 * smaller than when it was saved). Null for no share or one that is not
 * between 0 and 1, meaning the default layout.
 */
export function restoredShare(share: number | null | undefined, length: number, min = MIN_PANE_PIXELS): number | null {
  if (share === null || share === undefined || !(share > 0 && share < 1)) return null;
  return splitFraction(share * length, length, min);
}

/** A remembered pane name, if it is one. */
export function paneName(name: string | null | undefined): PaneName | null {
  return name === "editor" || name === "viewport" || name === "panel" ? name : null;
}

/** Every diagnostics severity, in the order saved. */
const SEVERITY_NAMES: readonly SeverityName[] = ["error", "warning", "info"];

/** The severities to show, from the remembered unticked ones (unknown
 *  names are ignored). */
export function shownSeverities(hidden: readonly string[] | undefined): Set<SeverityName> {
  const off = new Set(hidden ?? []);
  return new Set(SEVERITY_NAMES.filter((n) => !off.has(n)));
}

/** What the UI shows now, as far as it is remembered. */
export interface LayoutState {
  readonly editor: number | null;
  readonly panel: number | null;
  readonly maximised: PaneName | null;
  readonly legendFolded: boolean;
  readonly shown: ReadonlySet<SeverityName>;
}

/** The layout to remember, leaving out what is at its default. */
export function layoutToSave(state: LayoutState): PaneLayout {
  const hidden = SEVERITY_NAMES.filter((n) => !state.shown.has(n));
  return {
    ...(state.editor === null ? {} : { editor: state.editor }),
    ...(state.panel === null ? {} : { panel: state.panel }),
    ...(state.maximised === null ? {} : { maximised: state.maximised }),
    ...(state.legendFolded ? { legend_folded: true } : {}),
    ...(hidden.length === 0 ? {} : { hidden_diagnostics: hidden }),
  };
}
