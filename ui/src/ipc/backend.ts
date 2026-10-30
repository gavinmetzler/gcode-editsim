// Typed calls to the app's Rust commands (app/shell/src/lib.rs,
// ARCHITECTURE.md section 8.4a). Bulk data is raw bytes both ways: the
// program goes as the raw request body, the render buffer comes back as an
// ArrayBuffer. No G-code logic here.

import { invoke, isTauri } from "@tauri-apps/api/core";

/** A dialect the user can choose. */
export interface DialectEntry {
  readonly id: string;
  readonly name: string;
  readonly stub: boolean;
}

/** Which machine file. */
export interface MachineRef {
  readonly kind: "shipped" | "user";
  readonly id: string;
}

/** A machine file the user can choose. */
export interface MachineEntry {
  readonly label: string;
  readonly source: MachineRef;
  readonly problem: string | null;
}

/** `true` inside the desktop app; `false` in a plain browser (`npm run dev`). */
export function inApp(): boolean {
  return isTauri();
}

/** The dialects, in the order to offer them. */
export function listDialects(): Promise<DialectEntry[]> {
  return invoke<DialectEntry[]>("list_dialects");
}

/** The shipped machine files, then the user's. */
export function listMachines(): Promise<MachineEntry[]> {
  return invoke<MachineEntry[]>("list_machines");
}

/** The user's machine folder (created if missing). */
export function machineFolder(): Promise<string> {
  return invoke<string>("machine_folder");
}

/** Selects the machine for later runs; returns its name. */
export function selectMachine(source: MachineRef): Promise<string> {
  return invoke<string>("select_machine", { source });
}

/** Sets the dialect and options for later runs. */
export function setRunOptions(dialect: string, blockDelete: boolean, tolerance?: number): Promise<void> {
  return invoke<undefined>("set_run_options", {
    dialect,
    blockDelete,
    ...(tolerance === undefined ? {} : { tolerance }),
  });
}

/** Interprets `program` (raw bytes) and returns the render buffer. */
export async function interpret(program: Uint8Array, fileName: string): Promise<ArrayBuffer> {
  const result = await invoke<ArrayBuffer>("interpret", program, {
    headers: { "x-file-name": encodeURIComponent(fileName) },
  });
  if (!(result instanceof ArrayBuffer)) {
    throw new Error("interpret did not return raw bytes");
  }
  return result;
}
