// On Linux, a middle-button press and release pastes the "primary
// selection" (the last text selected anywhere) into the focused text field.
// In the 3D view the middle button zooms, so the view cancels that default
// action. The orbit controls use pointer events, which this does not touch.

/** The events whose default action, for the middle button, is to paste. */
export const MIDDLE_BUTTON_EVENTS = ["mousedown", "mouseup", "auxclick"] as const;

/** Stops middle-button presses on `target` from pasting. */
export function blockMiddleClickPaste(target: Pick<EventTarget, "addEventListener">): void {
  for (const type of MIDDLE_BUTTON_EVENTS) {
    target.addEventListener(type, (e) => {
      if ((e as MouseEvent).button === 1) e.preventDefault();
    });
  }
}
