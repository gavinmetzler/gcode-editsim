// Whether the toolpath, diagnostics and line sync shown match the editor's
// text. Edits do not re-run the program (on a large program that makes
// typing feel slow, and half-typed text is often broken); they mark what is
// shown as out of date until the user refreshes. Pure, so it is tested
// without a browser.

/** Counts edits and remembers which edit the shown result belongs to. */
export class Freshness {
  private edits = 0;
  private shown = 0;

  /** The text changed. */
  edited(): void {
    this.edits += 1;
  }

  /** A run is starting from the text as it is now. Returns its token, to
   *  pass to `shown` when its result is displayed. */
  runStarted(): number {
    return this.edits;
  }

  /** The result of the run with `token` is now displayed. */
  resultShown(token: number): void {
    this.shown = token;
  }

  /** True when the text changed after the run whose result is displayed
   *  started. */
  get stale(): boolean {
    return this.edits !== this.shown;
  }
}
