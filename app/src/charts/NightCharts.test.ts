import { describe, expect, it } from "vitest";
import { stageRuns, nightHypnogramScale, NIGHT_CHART_AXIS_WIDTH, NIGHT_CHART_PADDING } from "./NightCharts";

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

describe("Night chart alignment", () => {
  it("uses the actual device-pixel uPlot bounds and last-sample scale", () => {
    expect(nightHypnogramScale({ bbox: { left: 168, width: 1000, top: 0, height: 200 }, scales: { x: { min: 1800, max: 3599 } } }, 2)).toEqual({ left: 84, right: 584, min: 1800, max: 3599 });
    expect(NIGHT_CHART_AXIS_WIDTH).toBe(84);
    expect(NIGHT_CHART_PADDING).toEqual([8, 12, 0, 0]);
  });
});
