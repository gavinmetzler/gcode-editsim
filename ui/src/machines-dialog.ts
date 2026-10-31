// The machine files dialog: where the user's own machine files go, and a
// button to reload the list after adding or editing one.

import { el } from "./dom";

/** Opens a modal dialog showing the machine `folder`. `reload` reloads the
 *  machine list and returns a short result to show. */
export function showMachinesDialog(folder: string, reload: () => Promise<string>): void {
  const result = el("p", { className: "note" });
  const reloadButton = el("button", { type: "button", textContent: "Reload list" });
  const close = el("button", { type: "button", textContent: "Close", autofocus: true });
  const path = el("input", { type: "text", value: folder, readOnly: true, className: "path" });
  const dialog = el(
    "dialog",
    { className: "app-dialog machines-dialog" },
    el("h2", { textContent: "Machine files" }),
    el("p", {
      textContent:
        "Your machine files (.toml) are in this folder; the app put the example mill and router files there to start from. Edit them or add your own, then reload the list. The format is described in docs/machine-definition.md.",
    }),
    path,
    result,
    el("div", { className: "buttons" }, reloadButton, close),
  );
  path.addEventListener("focus", () => {
    path.select();
  });
  reloadButton.addEventListener("click", () => {
    reloadButton.disabled = true;
    result.textContent = "Reloading…";
    void reload()
      .then((text) => {
        result.textContent = text;
      })
      .catch((e: unknown) => {
        result.textContent = `Could not reload: ${String(e)}`;
      })
      .finally(() => {
        reloadButton.disabled = false;
      });
  });
  close.addEventListener("click", () => {
    dialog.close();
  });
  dialog.addEventListener("close", () => {
    dialog.remove();
  });
  document.body.append(dialog);
  dialog.showModal();
}
