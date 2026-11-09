import { afterEach, describe, expect, it, vi } from "vitest";
import { clearRhythmProfile, EMPTY_RHYTHM, focusOverlapsSleep, readRhythmProfile, rhythmCharacter, rhythmErrors, rhythmLabel, saveRhythmProfile, windowMinutes, type RhythmDraft } from "./rhythm";
const answers: RhythmDraft = { preference: "earlier", bedtime: "23:30", wakeTime: "07:00", schedule: "regular", alarm: "free", focusStart: "09:00", focusEnd: "11:00" };
afterEach(() => { clearRhythmProfile(false); vi.unstubAllGlobals(); });

describe("optional rhythm profile", () => {
  it("validates times, including overnight sleep and focus windows", () => {
    expect(windowMinutes("23:30", "07:00")).toBe(450);
    expect(windowMinutes("23:00", "01:00")).toBe(120);
    for (const time of ["", "24:00", "12:60", "9:00", "NaN"]) expect(windowMinutes(time, "08:00")).toBeNull();
    expect(windowMinutes("08:00", "08:00")).toBeNull();
    expect(rhythmErrors(EMPTY_RHYTHM)).toHaveLength(5);
    expect(rhythmErrors(answers)).toEqual([]);
  });
  it("flags planned focus inside the reported sleep window without inferring biology", () => {
    expect(focusOverlapsSleep(answers)).toBe(false);
    expect(focusOverlapsSleep({ ...answers, focusStart: "01:00", focusEnd: "02:00" })).toBe(true);
    expect(focusOverlapsSleep({ ...answers, focusStart: "22:00", focusEnd: "01:00" })).toBe(true);
    expect(focusOverlapsSleep({ ...answers, focusStart: "07:00", focusEnd: "09:00" })).toBe(false);
  });
  it("classifies only explicit preference, independently of schedule or score", () => {
    expect(["earlier", "middle", "later", "varies"].map(p => rhythmLabel(p as RhythmDraft["preference"])))
      .toEqual(["Morning-leaning", "Daytime-leaning", "Evening-leaning", "Mixed rhythm"]);
    expect(rhythmErrors({ ...answers, schedule: "variable", alarm: "constrained" })).toEqual([]);
  });
  it("pairs each reported preference with its character, keeping the base fox before an answer", () => {
    expect(["earlier", "middle", "later", "varies", ""].map(p => rhythmCharacter(p as RhythmDraft["preference"])))
      .toEqual(["./mascots/rhythm-fox-morning.png", "./mascots/rhythm-fox-daytime.png", "./mascots/rhythm-fox-evening.png", "./mascots/rhythm-fox-mixed.png", "./mascots/rhythm-fox.png"]);
    for (const value of ["unknown", "constructor", "__proto__"]) {
      expect(rhythmCharacter(value as RhythmDraft["preference"])).toBe("./mascots/rhythm-fox.png");
    }
  });
  it("keeps demo answers in memory and clears them without touching storage", () => {
    const storage = { setItem: vi.fn(), getItem: vi.fn(), removeItem: vi.fn() };
    vi.stubGlobal("localStorage", storage);
    saveRhythmProfile(answers, false);
    const profile = readRhythmProfile(false)!;
    profile.preference = "later";
    expect(readRhythmProfile(false)?.preference).toBe("earlier");
    clearRhythmProfile(false);
    expect(readRhythmProfile(false)).toBeNull();
    expect(storage.setItem).not.toHaveBeenCalled();
    expect(storage.removeItem).not.toHaveBeenCalled();
  });
  it("accepts only valid versioned saved profiles and clears native storage", () => {
    let saved: string | null = null;
    const removeItem = vi.fn(() => { saved = null; });
    vi.stubGlobal("localStorage", { getItem: () => saved, setItem: (_k: string, v: string) => { saved = v; }, removeItem });
    saveRhythmProfile(answers, true);
    expect(readRhythmProfile(true)?.version).toBe(1);
    for (const bad of ["{", "null", JSON.stringify({ ...answers, version: 2 }), JSON.stringify({ ...answers, version: 1, wakeTime: "25:00" }), JSON.stringify({ ...answers, version: 1, preference: "invented" })]) {
      saved = bad; expect(readRhythmProfile(true)).toBeNull();
    }
    saveRhythmProfile(answers, true);
    clearRhythmProfile(true);
    expect(removeItem).toHaveBeenCalledWith("sleeplens.rhythm.v1");
    expect(readRhythmProfile(true)).toBeNull();
  });
  it("reports saving failures instead of pretending the profile was saved", () => {
    vi.stubGlobal("localStorage", { setItem: () => { throw new Error("Storage is unavailable"); } });
    expect(() => saveRhythmProfile(answers, true)).toThrow("Storage is unavailable");
    expect(() => saveRhythmProfile(EMPTY_RHYTHM, false)).toThrow("Complete the questions");
  });
});
