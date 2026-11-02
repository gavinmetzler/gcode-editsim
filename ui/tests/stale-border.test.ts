import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

// M5.1: the red border that says the 3D view is out of date was hidden in
// the desktop app. It was an `outline` on the view's box, painted into the
// box's own layer; WebKitGTK draws the WebGL canvas in a layer of its own
// on top, so the outline showed only while the canvas was being resized.
// The border must be a positioned box drawn above the canvas instead.
// (The browser check is in the PR; this guards the stylesheet against
// going back to the outline.)

const css = readFileSync(new URL("../src/style.css", import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

/** The declarations of the first rule whose selector is exactly `selector`. */
function rule(selector: string): Map<string, string> | null {
  for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = (match[1] ?? "").split(",").map((s) => s.trim());
    if (!selectors.includes(selector)) continue;
    const declarations = new Map<string, string>();
    for (const part of (match[2] ?? "").split(";")) {
      const colon = part.indexOf(":");
      if (colon > 0) declarations.set(part.slice(0, colon).trim(), part.slice(colon + 1).trim());
    }
    return declarations;
  }
  return null;
}

describe("the out-of-date border", () => {
  it("is a positioned box over the whole view, drawn after the canvas", () => {
    const border = rule(".viewport.stale::after");
    expect(border).not.toBeNull();
    expect(border?.get("content")).toBe('""');
    expect(border?.get("position")).toBe("absolute");
    expect(border?.get("inset")).toBe("0");
    expect(border?.get("border")).toMatch(/^3px solid #e5484d$/);
    expect(border?.get("z-index")).toBeDefined();
  });

  it("lets clicks and drags through to the view", () => {
    expect(rule(".viewport.stale::after")?.get("pointer-events")).toBe("none");
  });

  it("is not an outline on the view's own box, which the canvas covers", () => {
    expect(rule(".viewport.stale")?.has("outline") ?? false).toBe(false);
  });
});
