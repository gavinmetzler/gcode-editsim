import { describe, expect, it } from "vitest";

import { Freshness } from "../src/freshness";

describe("Freshness", () => {
  it("starts up to date", () => {
    expect(new Freshness().stale).toBe(false);
  });

  it("is out of date after an edit, until a run from the new text is shown", () => {
    const f = new Freshness();
    f.edited();
    expect(f.stale).toBe(true);
    const run = f.runStarted();
    f.resultShown(run);
    expect(f.stale).toBe(false);
  });

  it("stays out of date if the text changed while the run was going", () => {
    const f = new Freshness();
    f.edited();
    const run = f.runStarted();
    f.edited();
    f.resultShown(run);
    expect(f.stale).toBe(true);
    f.resultShown(f.runStarted());
    expect(f.stale).toBe(false);
  });

  it("counts every edit", () => {
    const f = new Freshness();
    const before = f.runStarted();
    f.edited();
    f.edited();
    f.resultShown(before);
    expect(f.stale).toBe(true);
  });
});
