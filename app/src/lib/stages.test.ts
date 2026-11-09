import { describe, expect, it } from "vitest";
import { roundedStageDuration, displayStage, displayStageMinutes, stagePattern, readStageMode, countSleepCycles, stageRuns, stageShare, stageRangeStatus, TYPICAL_STAGE_RANGES } from "./stages";

describe("sleep cycle estimate", () => {
  it("counts consecutive REM epochs as a single period", () => {
    expect(countSleepCycles(["light", "rem", "rem", "deep", "rem"])).toBe(2);
  });
  it("uses at least one cycle when there is sleep", () => {
    expect(countSleepCycles(["wake", "light", "deep"])).toBe(1);
    expect(countSleepCycles(["rem"])).toBe(1);
  });
  it("returns zero for missing data or only wake", () => {
    expect(countSleepCycles([])).toBe(0);
    expect(countSleepCycles(["wake", "noData", "wake"])).toBe(0);
  });
  it("does not merge REM periods across missing data", () => {
    expect(countSleepCycles(["rem", "noData", "rem"])).toBe(2);
    expect(stageRuns(["rem", "noData", "rem"])).toHaveLength(3);
  });
});

describe("typical adult stage ranges", () => {
  it("uses the reference ranges", () => {
    expect(TYPICAL_STAGE_RANGES).toEqual({ wake: [0, 10], nonRem: [65, 80], rem: [20, 25], light: [45, 60], deep: [13, 23] });
  });
  it("includes both endpoints and classifies estimates outside the band", () => {
    expect(stageRangeStatus("rem", 20)).toBe("Within typical range");
    expect(stageRangeStatus("rem", 25)).toBe("Within typical range");
    expect(stageRangeStatus("rem", 19.9)).toBe("Below typical range");
    expect(stageRangeStatus("rem", 25.1)).toBe("Above typical range");
  });
  it("computes shares using all recorded stages, with empty nights safe", () => {
    expect(stageShare({ wake: 10, light: 50, deep: 20, rem: 20, uncertain: 0 }, "rem")).toBe(20);
    expect(stageShare({ wake: 0, light: 0, deep: 0, rem: 0, uncertain: 0 }, "wake")).toBe(0);
  });
});

describe("honest stage modes", () => {
  it("maps light and deep to Non-REM only in simple mode", () => {
    expect(["light", "deep"].map(stage => displayStage(stage as "light" | "deep", "simple"))).toEqual(["nonRem", "nonRem"]);
    expect(displayStage("deep", "detailed")).toBe("deep");
    expect(displayStageMinutes({ wake: 10, light: 45, deep: 15, rem: 20, uncertain: 10 }, "simple")).toEqual({ wake: 10, nonRem: 60, rem: 20, uncertain: 10 });
  });
  it("keeps uncertain and no-data visually and semantically distinct", () => {
    expect(displayStage("uncertain", "simple")).toBe("uncertain");
    expect(stagePattern("uncertain")).toBe("hatched");
    expect(stagePattern("noData")).toBe("empty-dashed");
    expect(stagePattern("nonRem")).toBe("solid");
    expect(stageShare({ wake: 10, light: 45, deep: 15, rem: 20, uncertain: 10 }, "uncertain")).toBe(10);
  });
  it("defaults to simple without browser storage", () => { expect(readStageMode()).toBe("simple"); });
});

describe("approximate stage durations", () => {
  it("uses five minute rounding then the same hour boundary for every stage", () => {
    expect(roundedStageDuration(44.2)).toBe("about 45 min");
    expect(roundedStageDuration(56)).toBe("about 55 min");
    expect(roundedStageDuration(59)).toBe("about 1 h");
    expect(roundedStageDuration(60)).toBe("about 1 h");
    expect(roundedStageDuration(74)).toBe("about 1 ¼ h");
    expect(roundedStageDuration(374)).toBe("about 6 ¼ h");
  });
});
