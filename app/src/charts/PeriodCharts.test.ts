import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PeriodBars } from "./PeriodCharts";

const points = [{ date: "2026-11-08", value: 180, deep: 90, rem: 90 }];
describe("period chart axes", () => {
  it("leaves room for hour labels on the deep and REM axis", () => {
    const html = renderToStaticMarkup(createElement(PeriodBars, { points, week: false, onPick: () => {} }));
    expect(html).toContain('x1="70"');
    expect(html).toContain('x="62" y="112" text-anchor="end">1.5 h</text>');
    expect(html).toContain('>3 h</text>');
  });
  it("names the bottom score guide Poor", () => {
    const html = renderToStaticMarkup(createElement(PeriodBars, { points, scores: true, week: false, onPick: () => {} }));
    expect(html).toContain('>Poor</text>');
    expect(html).not.toContain('>40</text>');
  });
});

describe("period chart stage modes", () => {
  it("shows only REM bars and descriptions in simple mode", () => {
    const html = renderToStaticMarkup(createElement(PeriodBars, { points: [{ ...points[0]!, value: 90 }], stageMode: "simple", week: false, onPick: () => {} }));
    expect(html).toContain("Estimated REM sleep per night.");
    expect(html).toContain("REM 1 h 30 m");
    expect(html).not.toContain("Deep");
    expect(html).not.toContain("var(--stage-deep)");
    expect(html).toContain("var(--stage-rem)");
  });
  it("retains stacked Deep and REM in detailed mode", () => {
    const html = renderToStaticMarkup(createElement(PeriodBars, { points, stageMode: "detailed", week: false, onPick: () => {} }));
    expect(html).toContain("Estimated deep and REM sleep per night, stacked bars.");
    expect(html).toContain("Deep 1 h 30 m, REM 1 h 30 m");
    expect(html).toContain("var(--stage-deep)");
    expect(html).toContain("var(--stage-rem)");
  });
});
