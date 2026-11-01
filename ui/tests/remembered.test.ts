import { describe, expect, it } from "vitest";

import type { DialectEntry, MachineEntry, MachineRef, Remembered } from "../src/ipc/backend";
import { startChoice } from "../src/remembered";

const user = (path: string): MachineRef => ({ kind: "user", id: path });
const machine = (label: string, source: MachineRef, problem: string | null = null): MachineEntry => ({
  label,
  source,
  problem,
});

const MILL = machine("Generic 3-axis VMC", user("/cfg/machines/generic_3axis_mill.toml"));
const ROUTER = machine("Generic 3-axis router", user("/cfg/machines/generic_3axis_router.toml"));
const BROKEN = machine("broken.toml", user("/cfg/machines/broken.toml"), "machine file: bad TOML");
const MACHINES = [MILL, ROUTER, BROKEN];

const DIALECTS: DialectEntry[] = [
  { id: "fanuc", name: "Fanuc (milling)", stub: false },
  { id: "osai", name: "Osai 10 Series (milling, early stub)", stub: true },
];

const NOTHING: Remembered = { machine: null, dialect: null, problem: null };

describe("startChoice", () => {
  it("starts with the first usable machine and the first dialect when nothing is remembered", () => {
    expect(startChoice(MACHINES, DIALECTS, NOTHING)).toEqual({ machine: 0, dialect: "fanuc", note: null });
  });

  it("starts with the remembered machine and dialect", () => {
    const remembered: Remembered = { machine: ROUTER.source, dialect: "osai", problem: null };
    expect(startChoice(MACHINES, DIALECTS, remembered)).toEqual({ machine: 1, dialect: "osai", note: null });
  });

  it("finds a remembered shipped example", () => {
    const shipped = machine("Generic 3-axis router", { kind: "shipped", id: "generic_3axis_router" });
    const list = [machine("Generic 3-axis VMC", { kind: "shipped", id: "generic_3axis_mill" }), shipped];
    const remembered: Remembered = { machine: { kind: "shipped", id: "generic_3axis_router" }, dialect: null, problem: null };
    expect(startChoice(list, DIALECTS, remembered).machine).toBe(1);
  });

  it("falls back to the first usable machine, and says so, when the remembered file is gone", () => {
    const remembered: Remembered = { machine: user("/cfg/machines/old router.toml"), dialect: "fanuc", problem: null };
    const choice = startChoice(MACHINES, DIALECTS, remembered);
    expect(choice.machine).toBe(0);
    expect(choice.dialect).toBe("fanuc");
    expect(choice.note).toBe(
      "The machine you used last (old router.toml) is no longer in the machine folder; using Generic 3-axis VMC.",
    );
  });

  it("falls back, and says why, when the remembered file has a problem now", () => {
    const remembered: Remembered = { machine: BROKEN.source, dialect: null, problem: null };
    const choice = startChoice(MACHINES, DIALECTS, remembered);
    expect(choice.machine).toBe(0);
    expect(choice.note).toBe(
      "The machine you used last (broken.toml) cannot be used (machine file: bad TOML); using Generic 3-axis VMC.",
    );
  });

  it("falls back to the first dialect when the remembered one is not available", () => {
    const remembered: Remembered = { machine: MILL.source, dialect: "haas", problem: null };
    expect(startChoice(MACHINES, DIALECTS, remembered)).toEqual({
      machine: 0,
      dialect: "fanuc",
      note: 'The dialect you used last ("haas") is not available; using Fanuc (milling).',
    });
  });

  it("passes on a problem with the settings file", () => {
    const remembered: Remembered = { machine: null, dialect: null, problem: "settings.json: not valid JSON" };
    expect(startChoice(MACHINES, DIALECTS, remembered)).toEqual({
      machine: 0,
      dialect: "fanuc",
      note: "Could not use all the saved settings (settings.json: not valid JSON).",
    });
  });

  it("joins several notes", () => {
    const remembered: Remembered = { machine: user("/x/gone.toml"), dialect: "haas", problem: "bad" };
    const note = startChoice(MACHINES, DIALECTS, remembered).note ?? "";
    expect(note.split(". ").length).toBe(3);
  });

  it("chooses no machine when none is usable", () => {
    const remembered: Remembered = { machine: BROKEN.source, dialect: null, problem: null };
    const choice = startChoice([BROKEN], DIALECTS, remembered);
    expect(choice.machine).toBe(-1);
    expect(choice.note).toBe(
      "The machine you used last (broken.toml) cannot be used (machine file: bad TOML), and no other machine file is usable.",
    );
    expect(startChoice([BROKEN], DIALECTS, NOTHING)).toEqual({ machine: -1, dialect: "fanuc", note: null });
  });
});
