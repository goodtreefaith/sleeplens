import { createElement } from "react";
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { SleepScore, SleepStageEstimate } from "../api/types";
import { scoreBreakdown } from "../lib/score";
import { glossaryEntry } from "../lib/glossary";
import { Hypnogram, roundedStageDuration, ScoreBreakdown, ScoreCard, StageSummary } from "./SleepQuality";
const score: SleepScore = { total: 80, label: "Good", estimated: true, components: [
  { key: "duration", label: "Hours used", points: 24, maxPoints: 30, detail: "Six hours of therapy." },
  { key: "maskSeal", label: "Mask seal", points: 15, maxPoints: 15, detail: "The mask sealed well." }
] };
describe("sleep quality rendering", () => {
  it("uses one three-step meter per contributor with category colours matching the ring", () => {
    const html = renderToStaticMarkup(createElement(ScoreCard, { score, go: () => {}, summary: "Sample night" }));
    expect(html).not.toContain("score-contributor-bar");
    const rows = html.split('class="score-contributor-reading"').slice(1);
    expect(rows).toHaveLength(5);
    expect(rows[0]!.split('</span>')[0]!.match(/data-filled="true"/g)).toHaveLength(2);
    expect(rows[1]!.split('</span>')[0]!.match(/data-filled="false"/g)).toHaveLength(3);
    expect(rows[2]!.split('</span>')[0]!.match(/data-filled="true"/g)).toHaveLength(3);
    expect(html).toContain('stroke="var(--score-duration)"');
    expect(html).toContain('class="score-component-dot" style="background:var(--score-duration)"');
    expect(html).toContain("Meter: Poor / Sufficient / Optimal");
    expect(html).toContain("REM sleep (estimated, low confidence)");
    expect(html).toContain('aria-label="What is REM sleep score?"');
    expect(html).toMatch(/<\/button><span class="score-rem-help"><span class="help-wrap"/);
    expect(glossaryEntry("remSleepScore")?.body.join(" ")).toContain("Deep sleep is not scored because it is the least reliable");
  });
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
    const stages: SleepStageEstimate = { start: "2026-11-08T23:00:00", epochSeconds: 30, stages: ["wake", "light", "noData", "deep", "rem"], minutes: { wake: .5, light: .5, deep: .5, rem: .5, uncertain: 0 }, confidence: { sleepWake: "moderate", rem: "low", deep: "veryLow" }, signalQuality: .8, estimatorVersion: 1 };
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages }));
    expect(html).toContain("23:00"); expect(html).toContain("No data"); expect(html).toContain("No data · empty dashed outline");
    const summary = renderToStaticMarkup(createElement(StageSummary, { minutes: stages.minutes }));
    expect(summary.match(/<b>25%<\/b>/g)).toHaveLength(2);
    expect(summary).toContain("<b>50%</b>");
  });
});

it("rounds stage estimates to useful approximate durations", () => {
  expect(roundedStageDuration(374)).toBe("about 6 ¼ h");
  expect(roundedStageDuration(31.5)).toBe("about 30 min");
  expect(roundedStageDuration(90)).toBe("about 1 ½ h");
});

it("uses the same rounded duration for uncertain time and REM", () => {
  const html = renderToStaticMarkup(createElement(StageSummary, { minutes: { wake: 45, light: 100, deep: 30, rem: 59, uncertain: 60 } }));
  expect(html.match(/about 1 h/g)).toHaveLength(2);
  expect(html).toContain("about 45 min");
  expect(html).not.toContain(">60 min<");
});
