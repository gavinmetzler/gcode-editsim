// The middle button must not paste the Linux primary selection when it is
// used to zoom the 3D view (owner's report, 2026-10-31). The paste itself
// cannot happen in a test browser, so this checks that the view cancels
// the default action of every middle-button mouse event.

import { describe, expect, it } from "vitest";

import { MIDDLE_BUTTON_EVENTS, blockMiddleClickPaste } from "../src/viewport/middle-click";

function fakeTarget(): { target: Pick<EventTarget, "addEventListener">; fire: (type: string, button: number) => boolean } {
  const handlers = new Map<string, ((e: Event) => void)[]>();
  return {
    target: {
      addEventListener: (type: string, h: EventListenerOrEventListenerObject | null) => {
        if (typeof h === "function") handlers.set(type, [...(handlers.get(type) ?? []), h]);
      },
    },
    fire: (type, button) => {
      let prevented = false;
      const e = { button, preventDefault: () => (prevented = true) } as unknown as Event;
      for (const h of handlers.get(type) ?? []) h(e);
      return prevented;
    },
  };
}

describe("blockMiddleClickPaste", () => {
  it("cancels the default action of middle-button mouse events", () => {
    const { target, fire } = fakeTarget();
    blockMiddleClickPaste(target);
    for (const type of MIDDLE_BUTTON_EVENTS) expect(fire(type, 1), type).toBe(true);
    expect(MIDDLE_BUTTON_EVENTS).toContain("auxclick");
  });

  it("leaves the left and right buttons alone", () => {
    const { target, fire } = fakeTarget();
    blockMiddleClickPaste(target);
    for (const type of MIDDLE_BUTTON_EVENTS) {
      expect(fire(type, 0), type).toBe(false);
      expect(fire(type, 2), type).toBe(false);
    }
  });
});
