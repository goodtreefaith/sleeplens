import { describe, expect, it } from "vitest";
import { countSleepCycles, stageRuns, stageShare, stageRangeStatus, TYPICAL_STAGE_RANGES } from "./stages";

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
    expect(TYPICAL_STAGE_RANGES).toEqual({ wake: [0, 10], rem: [20, 25], light: [45, 60], deep: [13, 23] });
  });
  it("includes both endpoints and classifies estimates outside the band", () => {
    expect(stageRangeStatus("rem", 20)).toBe("Within typical range");
    expect(stageRangeStatus("rem", 25)).toBe("Within typical range");
    expect(stageRangeStatus("rem", 19.9)).toBe("Below typical range");
    expect(stageRangeStatus("rem", 25.1)).toBe("Above typical range");
  });
  it("computes shares using all recorded stages, with empty nights safe", () => {
    expect(stageShare({ wake: 10, light: 50, deep: 20, rem: 20 }, "rem")).toBe(20);
    expect(stageShare({ wake: 0, light: 0, deep: 0, rem: 0 }, "wake")).toBe(0);
  });
});
