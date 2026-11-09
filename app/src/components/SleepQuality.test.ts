import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { SleepScore, SleepStageEstimate } from "../api/types";
import { scoreBreakdown } from "../lib/score";
import { Hypnogram, ScoreBreakdown, StageSummary } from "./SleepQuality";
const score: SleepScore = { total: 80, label: "Good", estimated: true, components: [
  { key: "duration", label: "Hours used", points: 24, maxPoints: 30, detail: "Six hours of therapy." },
  { key: "maskSeal", label: "Mask seal", points: 15, maxPoints: 15, detail: "The mask sealed well." }
] };
describe("sleep quality rendering", () => {
  it("renders the backend weights and details without assuming five present components", () => {
    expect(scoreBreakdown(score).map((c) => c.percent)).toEqual([80, 100]);
    const html = renderToStaticMarkup(createElement(ScoreBreakdown, { score }));
    expect(html).toContain("24 / 30"); expect(html).toContain("width:80%"); expect(html).toContain("Six hours of therapy.");
    expect(html).toContain("scaled to 100");
  });
  it("clamps rounded or invalid bar values and handles missing scores", () => {
    expect(scoreBreakdown({ ...score, components: [{ ...score.components[0]!, points: 35 }, { ...score.components[1]!, maxPoints: 0 }] }).map((c) => c.percent)).toEqual([100, 0]);
    expect(renderToStaticMarkup(createElement(ScoreBreakdown, { score: null }))).toContain("at least 30 minutes");
  });
  it("shows stage gaps and uses unshifted wall-clock labels", () => {
    const stages: SleepStageEstimate = { start: "2026-11-08T23:00:00", epochSeconds: 30, stages: ["wake", "light", "noData", "deep", "rem"], minutes: { wake: .5, light: .5, deep: .5, rem: .5 }, signalQuality: .8, estimatorVersion: 1 };
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages }));
    expect(html).toContain("23:00"); expect(html).toContain("No data"); expect(html).toContain("stroke-dasharray");
    const summary = renderToStaticMarkup(createElement(StageSummary, { minutes: stages.minutes }));
    expect(summary.match(/25%/g)).toHaveLength(4);
  });
});
