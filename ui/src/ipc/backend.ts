// Typed calls to the app's Rust commands (app/shell/src/lib.rs,
// ARCHITECTURE.md section 8.4a). Bulk data is raw bytes both ways: the
// program goes as the raw request body, the render buffer comes back as an
// ArrayBuffer. No G-code logic here.

import { invoke, isTauri } from "@tauri-apps/api/core";

import type { PaneLayout } from "../layout/panes";

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

/** What the app remembers from the last session (M4.1). The machine or
 *  dialect may no longer be available: `startChoice` checks. */
export interface Remembered {
  readonly machine: MachineRef | null;
  readonly dialect: string | null;
  /** The panes as last left (M4.2); null when never saved. */
  readonly layout: PaneLayout | null;
  /** Why some or all of the saved settings could not be used. */
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

/** The machine and dialect chosen in the last session. Never fails on a
 *  missing or damaged settings file (`problem` says so instead). */
export function rememberedSettings(): Promise<Remembered> {
  return invoke<Remembered>("remembered_settings");
}

/** Remembers the user's choice of machine and dialect for the next session. */
export function rememberChoice(machine: MachineRef, dialect: string): Promise<void> {
  return invoke<undefined>("remember_choice", { machine, dialect });
}

/** Remembers the panes' sizes and the maximised pane for the next session. */
export function rememberLayout(layout: PaneLayout): Promise<void> {
  return invoke<undefined>("remember_layout", { layout });
}

/** Puts the window back to its default size and forgets the remembered
 *  layout (the caller resets the panes). */
export function resetLayout(): Promise<void> {
  return invoke<undefined>("reset_layout");
}

/** One axis of a point in program coordinates. */
export interface AxisValue {
  readonly letter: string;
  readonly value: number;
}

/** Transforms lines of `program` (raw bytes) as `params` says (the
 *  request built by transform/request.ts); returns the binary edit list.
 *  A transformation that cannot be done rejects with the reason. */
export async function transformProgram(program: Uint8Array, params: unknown): Promise<ArrayBuffer> {
  const result = await invoke<ArrayBuffer>("transform", program, {
    headers: { "x-params": encodeURIComponent(JSON.stringify(params)) },
  });
  if (!(result instanceof ArrayBuffer)) {
    throw new Error("transform did not return raw bytes");
  }
  return result;
}

/** A point picked in the view (machine coordinates) in the program
 *  coordinates of `line` of `program`. */
export function programPoint(program: Uint8Array, line: number, point: readonly number[]): Promise<AxisValue[]> {
  return invoke<AxisValue[]>("program_point", program, {
    headers: { "x-params": encodeURIComponent(JSON.stringify({ line, point })) },
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

/** Reads the file at `path` and returns its raw bytes. */
export async function loadFile(path: string): Promise<ArrayBuffer> {
  const result = await invoke<ArrayBuffer>("load_file", { path });
  if (!(result instanceof ArrayBuffer)) {
    throw new Error("load_file did not return raw bytes");
  }
  return result;
}

/** Writes `contents` to `path` (replacing the file in one step). */
export function saveFile(path: string, contents: Uint8Array): Promise<void> {
  return invoke<undefined>("save_file", contents, {
    headers: { "x-path": encodeURIComponent(path) },
  });
}
