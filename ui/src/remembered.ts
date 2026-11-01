// Which machine and dialect the app starts with (M4.1): the ones
// remembered from the last session when they can still be used, otherwise
// the first usable machine and the first dialect, with a note for the
// status line saying why. Pure, so it is tested without the app.

import { baseName } from "./files";
import type { DialectEntry, MachineEntry, MachineRef, Remembered } from "./ipc/backend";

/** What to start with. */
export interface StartChoice {
  /** Index into the usable machines (those without a problem, in list
   *  order); -1 when none is usable. */
  readonly machine: number;
  /** The dialect id. */
  readonly dialect: string;
  /** Why the remembered choice was not used, for the status line. */
  readonly note: string | null;
}

const sameMachine = (a: MachineRef, b: MachineRef): boolean => a.kind === b.kind && a.id === b.id;

/** A remembered machine's name when it is not in the list: its file name. */
const refName = (ref: MachineRef): string => (ref.kind === "user" ? baseName(ref.id) : ref.id);

/** The machine and dialect to start with, from the listed machines and
 *  dialects and what was remembered. */
export function startChoice(
  machines: readonly MachineEntry[],
  dialects: readonly DialectEntry[],
  remembered: Remembered,
): StartChoice {
  const notes: string[] = [];
  if (remembered.problem !== null) {
    notes.push(`Could not use all the saved settings (${remembered.problem}).`);
  }

  const usable = machines.filter((m) => m.problem === null);
  const first = usable[0];
  const fallback = first ? `; using ${first.label}.` : ", and no other machine file is usable.";
  let machine = first ? 0 : -1;
  const ref = remembered.machine;
  if (ref !== null) {
    const listed = machines.find((m) => sameMachine(m.source, ref));
    if (listed === undefined) {
      notes.push(`The machine you used last (${refName(ref)}) is no longer in the machine folder${fallback}`);
    } else if (listed.problem !== null) {
      notes.push(`The machine you used last (${listed.label}) cannot be used (${listed.problem})${fallback}`);
    } else {
      machine = usable.indexOf(listed);
    }
  }

  let dialect = dialects[0]?.id ?? "fanuc";
  const wanted = remembered.dialect;
  if (wanted !== null) {
    if (dialects.some((d) => d.id === wanted)) {
      dialect = wanted;
    } else {
      notes.push(`The dialect you used last (${JSON.stringify(wanted)}) is not available; using ${dialects[0]?.name ?? dialect}.`);
    }
  }

  return { machine, dialect, note: notes.length > 0 ? notes.join(" ") : null };
}
