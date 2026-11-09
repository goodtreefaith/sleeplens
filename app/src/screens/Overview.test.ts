import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PeriodSummary } from "../api/types";
import { PeriodOverview } from "./Overview";

const summary: PeriodSummary = {
  pressure: null, headline: "Sample period", clampedToFirstNight: true,
  period: "month", from: "2026-10-10", to: "2026-11-08", daysInRange: 30,
  nights: 7, validNights: 7, compliancePct: 23, avgUsageMinutes: 420,
  avgAhi: 2, avgSleepScore: 80, avgExcessLeakP95: 4, avgPressureP90: 8,
  avgStageMinutes: { wake: 30, light: 210, deep: 60, rem: 90, uncertain: 30 },
  previous: { avgUsageMinutes: null, avgAhi: null, avgSleepScore: null, compliancePct: null },
  bestNight: null, worstNight: null, scoreSeries: [], tips: []
};
const render = () => renderToStaticMarkup(createElement(PeriodOverview, { summary, stageMinutes: new Map(), unit: "cmH2O", go: () => {} }));
afterEach(() => vi.unstubAllGlobals());
describe("period stage presentation", () => {
  it("uses the saved Simple mode for the REM title and hides the Deep caveat", () => {
    const html = render();
    expect(html).toContain("<h2>REM (estimated)</h2>");
    expect(html).not.toContain("Deep + REM (estimated)");
    expect(html).not.toContain("Deep sleep is the least reliable part");
  });
  it("uses the saved Detailed mode for the stacked chart title and caveat", () => {
    vi.stubGlobal("window", { localStorage: { getItem: () => "detailed" } });
    const html = render();
    expect(html).toContain("<h2>Deep + REM (estimated)</h2>");
    expect(html).toContain("Deep sleep is the least reliable part of this estimate.");
  });
});
