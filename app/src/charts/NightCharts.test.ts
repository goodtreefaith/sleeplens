import { describe, expect, it } from "vitest";
import { stageRuns, nightHypnogramScale, NIGHT_CHART_AXIS_WIDTH, NIGHT_CHART_PADDING, minimumPressureMarkerPath } from "./NightCharts";

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

import { eventAtMinimum, pressureSettingBounds } from "./NightCharts";
import type { Settings, TherapyEvent } from "../api/types";

it("marks only known obstructive/hypopnea pressures within the minimum tolerance", () => {
  const event: TherapyEvent = { time: "2026-11-08T23:00:00", kind: "obstructiveApnea", value: 12, pressure: 5.5, code: 1 };
  expect(eventAtMinimum(event, 5)).toBe(true);
  expect(eventAtMinimum({ ...event, kind: "hypopnea", pressure: 4.7 }, 5)).toBe(true);
  expect(eventAtMinimum({ ...event, pressure: 5.51 }, 5)).toBe(false);
  expect(eventAtMinimum({ ...event, kind: "centralApnea" }, 5)).toBe(false);
  expect(eventAtMinimum({ ...event, pressure: null }, 5)).toBe(false);
  expect(eventAtMinimum(event, undefined)).toBe(false);
  expect(pressureSettingBounds({ settings: { minPressure: 5, maxPressure: 9 } as Settings })).toEqual([5, 9]);
  expect(pressureSettingBounds({ settings: { minPressure: 7, maxPressure: 7 } as Settings })).toEqual([7]);
  expect(pressureSettingBounds({ settings: null })).toEqual([]);
});

it("uses an open downward caret for minimum-pressure events", () => {
  expect(minimumPressureMarkerPath(9, 7, 6)).toBe("M3 4L9 10L15 4");
});
