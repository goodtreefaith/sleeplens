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
