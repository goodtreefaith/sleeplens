import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SleepStageEstimate } from "../api/types";
import { Hypnogram, StageSummary } from "./SleepStages";
import { wallSeconds } from "../lib/dates";

const stages: SleepStageEstimate = { start: "2026-11-08T23:00:00", epochSeconds: 1800, stages: ["wake", "light", "noData", "deep", "rem"], minutes: { wake: 30, light: 30, deep: 30, rem: 30 }, signalQuality: .8, estimatorVersion: 1 };

describe("shared sleep stages", () => {
  it("preserves the measured SVG aspect ratio over legacy fixed-height rules", () => {
    const css = readFileSync(new URL("../stages.css", import.meta.url), "utf8");
    expect(css).toContain(".hypnogram.stepped-hypnogram svg{height:auto}");
  });
  it("joins consecutive stage blocks and leaves gaps disconnected", () => {
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages }));
    expect(html).toContain('y1="22" y2="82"');
    expect(html).toContain('y1="112" y2="52"');
    expect(html).not.toContain('y1="82" y2="112"');
    expect(html).toContain('height="118" fill="url(');
    expect(html).toContain('rx="4"');
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
    const html = renderToStaticMarkup(createElement(Hypnogram, { stages,
      chartScale: { left: 92, right: 612, min: start, max: start + 7200 }
    }));
    expect(html).toContain('x1="92" x2="612"');
    expect(html).toContain('x="222" y="74" width="130"');
    expect(html).toContain('x="482" y="104" width="130"');
    expect(html).not.toContain('<title>REM,');
    expect(html).toContain('stroke="var(--stage-outline)"');
  });
  it("shows typical ranges, percentages, rounded durations and estimated cycles", () => {
    const html = renderToStaticMarkup(createElement(StageSummary, { minutes: stages.minutes, stages }));
    expect(html.match(/role="meter"/g)).toHaveLength(4);
    expect(html.match(/<b>25%<\/b>/g)).toHaveLength(4);
    expect(html).toContain('Typical adult range');
    expect(html).toContain('REM share. Typical adult range 20 to 25 percent. Within typical range.');
    expect(html).toContain('About 1 sleep');
    expect(html).toContain('not a sleep study');
  });
});
