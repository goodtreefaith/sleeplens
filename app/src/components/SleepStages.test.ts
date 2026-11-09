import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SleepStageEstimate } from "../api/types";
import { Hypnogram, StageSummary } from "./SleepStages";
import { wallSeconds } from "../lib/dates";

const stages: SleepStageEstimate = { start: "2026-11-08T23:00:00", epochSeconds: 1800, stages: ["wake", "light", "noData", "deep", "rem", "uncertain"], minutes: { wake: 30, light: 30, deep: 30, rem: 30, uncertain: 30 }, confidence: { sleepWake: "moderate", rem: "low", deep: "veryLow" }, signalQuality: .8, estimatorVersion: 3 };

describe("shared sleep stages", () => {
  it("preserves the measured SVG aspect ratio over legacy fixed-height rules", () => {
    const css = readFileSync(new URL("../stages.css", import.meta.url), "utf8");
    expect(css).toContain(".hypnogram.stepped-hypnogram svg{height:auto}");
  });
  it("defaults to simple rows and separates uncertain hatching from empty missing data", () => {
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages }));
    expect(html).toContain('data-stage="nonRem"');
    expect(html).not.toContain('data-stage="deep"');
    expect(html).toContain('data-stage="uncertain"');
    expect(html).toContain('fill="none" stroke="var(--muted)" stroke-dasharray="3 3"');
    expect(html).toContain('Uncertain · hatched');
    expect(html).toContain('No data · empty dashed outline');
    expect(html).toContain('aria-pressed="true">Simple');
  });
  it("joins stage blocks but breaks connectors across uncertain and missing time", () => {
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages, mode: "detailed" }));
    expect(html).toContain('y1="22" y2="52"');
    expect(html).toContain('y1="82" y2="112"');
    expect(html).not.toContain('y1="52" y2="82"');
    expect(html).not.toContain('y1="112" y2="142"');
    expect(html).toContain('Deep sleep is the least reliable part of this estimate.');
  });
  it("uses supplied zoom window for the Night lane", () => {
    const start = wallSeconds(stages.start);
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages, from: start + 3600, to: start + 7200, axisWidth: 84 }));
    expect(html).toContain('x1="84"');
    expect(html).toContain('>00:00</text>');
    expect(html).toContain('>01:00</text>');
    expect(html).not.toContain('<title>Awake,');
    expect(html).not.toContain('<title>REM,');
  });
  it("places stage transitions on the measured uPlot bounds and scale", () => {
    const start = wallSeconds(stages.start);
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages, chartScale: { left: 92, right: 612, min: start, max: start + 7200 } }));
    expect(html).toContain('x1="92" x2="612"');
    expect(html).toContain('x="222" y="44" width="130"');
    expect(html).toContain('x="482" y="44" width="130"');
    expect(html).not.toContain('<title>REM,');
    expect(html).toContain('stroke="var(--stage-outline)"');
  });
  it("summarizes simple Non-REM as light plus deep and shows uncertainty and confidence", () => {
    const html = renderToStaticMarkup(createElement(StageSummary, { minutes: stages.minutes, stages }));
    expect(html.match(/role="meter"/g)).toHaveLength(4);
    expect(html).toContain('<b>40%</b>');
    expect(html).toContain('Non-REM share. Typical adult range 65 to 80 percent.');
    expect(html).toContain('30 min');
    expect(html).toContain('How sure is this?');
    expect(html).toContain('Asleep vs awake');
    expect(html).toContain('Moderate');
    expect(html).not.toContain('Very low');
    expect(html).toContain('About 1 sleep');
    expect(html).toContain('not a sleep study');
    const detailed = renderToStaticMarkup(createElement(StageSummary, { minutes: stages.minutes, mode: "detailed" }));
    expect(detailed.match(/role="meter"/g)).toHaveLength(5);
    expect(detailed).toContain('Very low');
    expect(detailed).toContain('Deep sleep is the least reliable part of this estimate.');
  });
});
