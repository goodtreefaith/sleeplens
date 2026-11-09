import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TrendPoint } from "../api/types";
import { LineChart } from "./TrendCharts";

const point: TrendPoint = { date: "2026-11-08", ahi: 3, usageMinutes: 420, excessLeakMedian: 0, excessLeakP95: 16, sleepScore: 82, pressureP90: 8, settingsChanged: true, prescriptionChanged: false };
function chart(p: TrendPoint) {
  return renderToStaticMarkup(createElement(LineChart, { points: [p], from: p.date, to: p.date, value: (v: TrendPoint) => v.excessLeakP95, guide: 5, guideLabel: "5 = top of normal range", changeDescriptions: { [p.date]: "Minimum pressure: 7" }, unit: "L/min", aria: "Extra leak", onPick: () => {} }));
}
describe("trend chart annotations", () => {
  it("ignores comfort-only settings changes and keeps the guide explanation outside the plot", () => {
    const html = chart(point);
    expect(html).not.toContain('class="prescription-marker"');
    expect(html.slice(html.indexOf("<svg"))).not.toContain("top of normal range");
    expect(html).toContain("16.0 L/min");
  });
  it("makes prescription markers focusable with the date and actual settings", () => {
    const html = chart({ ...point, prescriptionChanged: true });
    expect(html).toContain('class="prescription-marker" tabindex="0"');
    expect(html).toContain("8 Nov 2026: Minimum pressure: 7");
    expect(html).not.toContain("Settings changed</text>");
  });
});
