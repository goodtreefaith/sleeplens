import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TrendPoint } from "../api/types";
import { LineChart, UsageCalendar, niceTicks } from "./TrendCharts";

const point: TrendPoint = { date: "2026-11-08", ahi: 3, usageMinutes: 420, excessLeakMedian: 0, excessLeakP95: 16, sleepScore: 82, pressureP90: 8, settingsChanged: true, prescriptionChanged: false };
function chart(p: TrendPoint) {
  return renderToStaticMarkup(createElement(LineChart, { points: [p], from: p.date, to: p.date, value: (v: TrendPoint) => v.excessLeakP95, typicalMax: 24, typicalLabel: "Typical range: under 24 L/min", changeDescriptions: { [p.date]: "Minimum pressure: 7" }, unit: "L/min", aria: "Extra leak", onPick: () => {} }));
}
describe("trend chart annotations", () => {
  it("ignores comfort-only settings changes and keeps the guide explanation outside the plot", () => {
    const html = chart(point);
    expect(html).not.toContain('class="prescription-marker"');
    expect(html.slice(html.indexOf("<svg"))).not.toContain("Typical range");
    expect(html).toContain("16.0 L/min");
  });
  it("shades the typical range without a dashed threshold", () => {
    const html = chart(point);
    expect(html).toContain('class="typical-range-band"');
    expect(html).toContain("Typical range: under 24 L/min");
    expect(html).not.toContain('class="guide"');
  });
  it("labels all four score levels with dotted guides", () => {
    const html = renderToStaticMarkup(createElement(LineChart, { points: [point], from: point.date, to: point.date, value: (v: TrendPoint) => v.sleepScore, scoreGuides: true, max: 100, unit: "points", aria: "Sleep score", onPick: () => {} }));
    for (const label of ["Poor", "50 Fair", "70 Good", "85 Great"]) expect(html).toContain(`>${label}</text>`);
    expect(html.match(/class="score-guide"/g)).toHaveLength(5);
    expect(html).not.toContain('class="gridline"');
  });
  it("includes prescription settings in the chart without adding marker tab stops", () => {
    const html = chart({ ...point, prescriptionChanged: true });
    expect(html).toContain('class="prescription-marker"');
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html).toContain("8 Nov 2026: Minimum pressure: 7");
    expect(html).not.toContain("Settings changed</text>");
  });
});

describe("trend chart navigation and axes", () => {
  it("always puts a y-axis tick at or above the highest value", () => {
    for (const max of [0.2, 3.2, 9, 18, 26, 53, 97, 101]) {
      const ticks = niceTicks(max);
      expect(ticks[0]).toBe(0);
      expect(ticks[ticks.length - 1]).toBeGreaterThanOrEqual(max);
    }
  });
  it("has one chart tab stop even with many nights and prescription markers", () => {
    const points = Array.from({ length: 30 }, (_, i) => ({ ...point, date: `2026-11-${String(i + 1).padStart(2, "0")}`, prescriptionChanged: true }));
    const html = renderToStaticMarkup(createElement(LineChart, { points, from: "2026-11-01", to: "2026-11-30", value: (p: TrendPoint) => p.ahi, unit: "events/h", aria: "AHI", onPick: () => {} }));
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html).toContain("Arrow keys select a night");
  });
  it("uses one tab stop for all calendar days, including days with no recording", () => {
    const html = renderToStaticMarkup(createElement(UsageCalendar, { points: [point], from: "2026-11-01", to: "2026-11-30", last: "2026-11-30", onPick: () => {} }));
    expect(html.match(/tabindex="0"/g)).toHaveLength(1);
    expect(html.match(/data-day=/g)).toHaveLength(30);
    expect(html).toContain('data-day="2026-11-01" tabindex="0"');
  });
});
