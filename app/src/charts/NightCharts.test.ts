import { describe, expect, it } from "vitest";
import { stageRuns } from "./NightCharts";

describe("hypnogram runs", () => {
  it("keeps gaps and stage transitions at their original epoch boundaries", () => {
    expect(stageRuns(["wake", "wake", "light", "deep", "deep", "noData", "noData", "rem"])).toEqual([
      { stage: "wake", first: 0, end: 2 },
      { stage: "light", first: 2, end: 3 },
      { stage: "deep", first: 3, end: 5 },
      { stage: "noData", first: 5, end: 7 },
      { stage: "rem", first: 7, end: 8 },
    ]);
    expect(stageRuns([])).toEqual([]);
  });
});
