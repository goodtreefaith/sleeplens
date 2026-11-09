import { describe, expect, it } from "vitest";
import type { StageMinutes } from "../api/types";
import { periodTickIndices, periodTitle, restorativePoints } from "./periodCharts";
const stage = (deep: number, rem: number): StageMinutes => ({ deep, rem, light: 200, wake: 20, uncertain: 0 });
describe("period chart data", () => {
  it("names the latest periods and dates older ones", () => {
    expect(periodTitle("week", "2026-11-03", "2026-11-09", "2026-11-09")).toBe("This week");
    expect(periodTitle("month", "2026-10-11", "2026-11-09", "2026-11-09")).toBe("This month");
    expect(periodTitle("6months", "2026-05-12", "2026-11-09", "2026-11-09")).toBe("Last 6 months");
    expect(periodTitle("year", "2025-11-10", "2026-11-09", "2026-11-09")).toBe("This year");
    expect(periodTitle("week", "2026-10-27", "2026-11-02", "2026-11-09")).toContain("2026");
  });
  it("keeps missing estimates as gaps in per-night bars", () => {
    const points = restorativePoints("2026-11-01", "2026-11-03", "week", new Map([["2026-11-01", stage(60, 90)], ["2026-11-03", null]]));
    expect(points).toHaveLength(3);
    expect(points[0]).toMatchObject({ deep: 60, rem: 90, nights: 1 });
    expect(points[1]).toMatchObject({ deep: null, rem: null, nights: 0 });
    expect(points[2]).toMatchObject({ deep: null, rem: null, nights: 0 });
  });
  it("averages estimated nights only, including partial weeks, and opens a contributing night", () => {
    const points = restorativePoints("2026-11-01", "2026-11-09", "6months", new Map([["2026-11-02", stage(60, 80)], ["2026-11-03", stage(100, 120)], ["2026-11-09", stage(30, 45)]]));
    expect(points).toEqual([{ date: "2026-11-01", end: "2026-11-07", deep: 80, rem: 100, nights: 2, pickDate: "2026-11-02" }, { date: "2026-11-08", end: "2026-11-09", deep: 30, rem: 45, nights: 1, pickDate: "2026-11-09" }]);
    expect(restorativePoints("2026-11-01", "2026-11-07", "year", new Map())[0]).toMatchObject({ deep: null, rem: null, pickDate: null });
  });
});

describe("period date ticks", () => {
  const labels = (count: number) => Array.from({ length: count }, (_, i) => i === count - 1 ? "4 Nov" : "14 Oct");
  it("removes the penultimate tick when it crowds the final month label", () => {
    expect(periodTickIndices(labels(30), false, 520)).toEqual([0, 7, 14, 21, 29]);
  });
  it("allows 7 pixels per character and 12 pixels between labels, keeping the final date", () => {
    for (const width of [40, 100, 280, 520, 900]) {
      for (const count of [1, 7, 26, 28, 30, 31, 53, 365]) {
        const week = count === 7;
        const text = labels(count);
        const ticks = periodTickIndices(text, week, width);
        const edges = (i: number) => {
          const x = (i + .5) * width / count, labelWidth = text[i]!.length * 7;
          const left = !week && i === 0 ? x : !week && i === count - 1 ? x - labelWidth : x - labelWidth / 2;
          return { left, right: left + labelWidth };
        };
        for (let i = 1; i < ticks.length; i++) expect(edges(ticks[i]!).left - edges(ticks[i - 1]!).right).toBeGreaterThanOrEqual(12);
        expect(ticks[ticks.length - 1]).toBe(count - 1);
      }
    }
    expect(periodTickIndices([], false, 520)).toEqual([]);
  });
  it("uses the actual shorter labels for compact week charts", () => {
    expect(periodTickIndices(["1", "2", "3", "4", "5", "6", "7"], true, 140)).toEqual([0, 1, 2, 3, 4, 5, 6]);
  });
});
