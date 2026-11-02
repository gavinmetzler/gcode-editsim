import { searchKeymap } from "@codemirror/search";
import { describe, expect, it } from "vitest";

import { FIND_TOOLTIP, SEARCH_PHRASES, findKeymap, findThemeSpec } from "../src/editor/find";

describe("Find and Replace (M5.3)", () => {
  it("names every key in the button's tooltip", () => {
    for (const key of ["Ctrl+F", "Ctrl+H", "F3", "Shift+F3"]) expect(FIND_TOOLTIP).toContain(key);
  });

  it("opens the panel at the Replace field with Ctrl+H, a key CodeMirror's search does not use", () => {
    const opened: boolean[] = [];
    const keys = findKeymap((replace) => {
      opened.push(replace);
      return true;
    });
    const ctrlH = keys.find((k) => k.key === "Mod-h");
    expect(ctrlH?.preventDefault).toBe(true);
    expect(ctrlH?.run?.({} as never)).toBe(true);
    expect(opened).toEqual([true]);
    expect(searchKeymap.some((k) => k.key === "Mod-h")).toBe(false);
  });

  it("capitalises the panel's labels like the app's buttons", () => {
    for (const phrase of ["next", "previous", "all", "match case", "regexp", "by word", "replace", "replace all"]) {
      const shown = SEARCH_PHRASES[phrase];
      expect(shown, phrase).toBeDefined();
      expect(shown?.toLowerCase()).toBe(phrase === "regexp" ? "regular expression" : phrase);
      expect(shown?.[0]).toBe(shown?.[0]?.toUpperCase());
    }
  });

  it("draws the panel's fields and buttons dark, like the rest of the app", () => {
    const field = findThemeSpec[".cm-textfield"];
    const button = findThemeSpec[".cm-button"];
    expect(field?.backgroundColor).toBe("#14171c");
    expect(field?.color).toBe("#e6e6e6");
    expect(button?.backgroundColor).toBe("#2b3038");
    expect(button?.backgroundImage).toBe("none");
    expect(findThemeSpec[".cm-panels"]?.backgroundColor).toBe("#1d2128");
  });

  it("shows matches in a colour the selection does not use", () => {
    const match = findThemeSpec[".cm-searchMatch"]?.backgroundColor;
    const current = findThemeSpec[".cm-searchMatch.cm-searchMatch-selected"]?.backgroundColor;
    expect(match).toBeDefined();
    expect(current).toBeDefined();
    expect(match).not.toBe(current);
    for (const c of [match, current]) expect(c).not.toBe("#3a4a63");
  });
});
