import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PressureInsights } from "../components/PressureInsights";
import type { Tip } from "../api/types";

const disclaimer = "SleepLens never changes settings. Only your clinician should change your pressure.";
const go = () => {};
const tip: Tip = { id: "pressure-ceiling", category: "pressure", severity: "info", title: "The top of the range was often used", body: "Mention this pattern at your next review.", evidence: "P90 near maximum on three nights", evidenceDates: ["2026-11-08"], learnSlug: "pressureInsights" };

describe("pressure insights card", () => {
  it("keeps pressure observations separate, links evidence nights and ends with clinician guidance", () => {
    const html = renderToStaticMarkup(createElement(PressureInsights, { tips: [tip, { ...tip, id: "therapy", category: "therapy", title: "Therapy-only suggestion" }], go }));
    expect(html).toContain(tip.title);
    expect(html).toContain("Information");
    expect(html).toContain(tip.evidence);
    expect(html).toContain('href="#night/2026-11-08"');
    expect(html).not.toContain("Therapy-only suggestion");
    expect(html).toContain(">8 Nov 2026</a>");
    expect(html).toMatch(new RegExp(`${disclaimer}</p></section>$`));
  });
  it("shows the period facts with help and a quiet fallback", () => {
    const html = renderToStaticMarkup(createElement(PressureInsights, { tips: [], unit: "hPa", nights: 30, summary: { settingMin: 5, settingMax: 9, avgP90: 7.4, nightsNearMax: 3, nightsNearMin: 2, avgShareAtMaxPct: 12.5, eventsAtMinPct: 31 }, go }));
    ["Prescribed range 5–9 hPa", "P90 averaged 7.4 hPa", "Near the maximum on 3 of 30 nights", "Time near max: 12.5%", "Events at the minimum pressure: 31.0%", "Nothing to flag about pressure in this period.", disclaimer].forEach(text => expect(html).toContain(text));
    expect(html.match(/class="help-tip"/g)?.length).toBe(6);
  });
});

it("distinguishes missing period evidence from a night without flags", () => {
  const render = (nights?: number) => renderToStaticMarkup(createElement(PressureInsights, { tips: [], nights, go }));
  expect(render()).toContain("Nothing to flag about pressure for this night.");
  for (const nights of [0, 1, 6]) expect(render(nights)).toContain("Not enough nights yet to look for pressure patterns (needs at least 7).");
  expect(render(7)).toContain("Nothing to flag about pressure in this period.");
});
