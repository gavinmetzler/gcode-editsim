import { describe, expect, it } from "vitest";

import { linesToMark } from "../src/editor/selected-lines";

describe("linesToMark", () => {
  it("marks the selected lines that are on screen", () => {
    expect(linesToMark({ first: 3, last: 6 }, [{ first: 1, last: 20 }])).toEqual([3, 4, 5, 6]);
  });

  it("marks only the part of a large selection that is on screen", () => {
    expect(linesToMark({ first: 1, last: 200_000 }, [{ first: 5000, last: 5004 }])).toEqual([
      5000, 5001, 5002, 5003, 5004,
    ]);
  });

  it("marks nothing when the selection is off screen", () => {
    expect(linesToMark({ first: 1, last: 10 }, [{ first: 50, last: 90 }])).toEqual([]);
  });

  it("marks nothing for a selection within one line (the browser shows it)", () => {
    expect(linesToMark({ first: 4, last: 4 }, [{ first: 1, last: 20 }])).toEqual([]);
    expect(linesToMark(null, [{ first: 1, last: 20 }])).toEqual([]);
  });

  it("handles several visible ranges in order, without repeats", () => {
    expect(
      linesToMark({ first: 2, last: 12 }, [
        { first: 1, last: 3 },
        { first: 3, last: 4 },
        { first: 10, last: 15 },
      ]),
    ).toEqual([2, 3, 4, 10, 11, 12]);
  });
});
