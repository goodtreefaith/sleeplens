/// <reference types="vite/client" />
import { describe, expect, it } from "vitest";
import contract from "../../../docs/APP_CONTRACT.md?raw";
import { GLOSSARY, GLOSSARY_GROUPS, glossaryEntry, HELP_KEYS } from "./glossary";

describe("shared glossary", () => {
  it("covers every help key in the app contract", () => {
    const section = contract.split("### Help terms")[1]!.split("###")[0]!;
    const keys = [...section.matchAll(/`([a-zA-Z0-9]+)`/g)].map((match) => match[1]!);
    expect(keys.length).toBe(20);
    expect([...HELP_KEYS].sort()).toEqual(keys.sort());
    keys.forEach((key) => { expect(glossaryEntry(key)?.body.join(" ").length).toBeGreaterThan(50); });
  });
  it("keeps existing Learn links and uses current extra-leak measurements", () => {
    ["hours", "obstructive", "central", "leak", "units", "ramp", "comfort", "humidifier", "computed"].forEach((key) => expect(glossaryEntry(key)).toBeDefined());
    expect(glossaryEntry("leak")?.body.join(" ")).toContain("95th percentile");
    expect(new Set(GLOSSARY.map((entry) => entry.id)).size).toBe(GLOSSARY.length);
  });
});

it("indexes every glossary entry once in the grouped Learn sections", () => {
  const keys = GLOSSARY_GROUPS.flatMap(group => group.keys);
  expect(keys.slice().sort()).toEqual(GLOSSARY.map(entry => entry.id).sort());
  expect(new Set(keys).size).toBe(keys.length);
});

it("explains v2.2 stage uncertainty and pressure concepts in the appropriate Learn groups", () => {
  const sleep = GLOSSARY_GROUPS.find(group => group.id === "sleep")!.keys;
  const settings = GLOSSARY_GROUPS.find(group => group.id === "settings")!.keys;
  ["uncertainStage", "nonRemSleep", "stageAccuracy"].forEach(key => expect(sleep).toContain(key));
  ["pressureInsights", "pressureP90", "pressureRange"].forEach(key => expect(settings).toContain(key));
  expect(GLOSSARY_GROUPS.find(group => group.id === "breathing")!.keys).toContain("treatmentEmergentCentralApnea");
  ["uncertainStage", "nonRemSleep", "stageAccuracy", "pressureInsights", "pressureP90", "pressureRange", "treatmentEmergentCentralApnea"].forEach(key => {
    const body = glossaryEntry(key)!.body.join(" ");
    const sentences = body.match(/[.!?](?:\s|$)/g) ?? [];
    expect(sentences.length).toBeGreaterThanOrEqual(2);
    expect(sentences.length).toBeLessThanOrEqual(3);
  });
  // These entries now cite SleepLens's own measurements rather than only the
  // literature. They must still say plainly that the stage split is unreliable.
  ["sleepWakeConfidence", "remConfidence", "deepConfidence"].forEach(key =>
    expect(glossaryEntry(key)!.body[0]).toMatch(/sleep-lab recordings|least reliable|often missed/));
  expect(glossaryEntry("remSleepScore")!.body.join(" ")).toContain("Deep sleep is not scored");
});
