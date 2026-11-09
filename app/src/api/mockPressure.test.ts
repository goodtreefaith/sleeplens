import { expect, it } from "vitest";
import { createMockApi } from "./mock";
import { MOCK_FIRST_DATE, mockNightDetail, mockNights, mockPressureObservation } from "./mockData";
import { pressureSummary, pressureTips, type PressureObservation } from "./mockPressure";
import { addDays, wallSeconds } from "../lib/dates";

it("covers pressure rules across synthetic nights and a ceiling week, without settings instructions", async () => {
  const observations = mockNights().flatMap(n => { const o = mockPressureObservation(n.date); return o ? [o] : []; });
  const tips = observations.flatMap(o => pressureTips([o]));
  const api = createMockApi({ preloaded: true });
  const week = await api.periodSummary("week", addDays(MOCK_FIRST_DATE, 106));
  for (const period of ["week", "month", "6months", "year"] as const) tips.push(...(await api.periodSummary(period)).tips.filter(t => t.category === "pressure"));
  const sampledRules = new Set<string>();
  for (const observation of observations) {
    const rules = pressureTips([observation]);
    if (rules.some(t => !sampledRules.has(t.id))) {
      tips.push(...mockNightDetail(observation.item.date)!.tips.filter(t => t.category === "pressure"));
      rules.forEach(t => sampledRules.add(t.id));
    }
  }
  const low = synthetic();
  low.item.pressureP90 = low.settings.minPressure;
  tips.push(...pressureTips([low]));
  tips.push(...pressureTips([{ ...synthetic(), highPressureLeakMean: 35, lowPressureLeakMean: 5 }]));
  const ids = new Set(tips.map(t => t.id));
  for (const id of ["pressure-ok", "pressure-ceiling-events", "pressure-ceiling", "pressure-floor-events", "pressure-low-use", "central-with-pressure", "leak-at-high-pressure"]) expect(ids.has(id), id).toBe(true);
  expect(observations.filter(o => pressureTips([o]).some(t => t.id === "pressure-ok")).length).toBeGreaterThan(observations.length / 2);
  expect(week.pressure?.nightsNearMax).toBe(7);
  expect(week.tips.some(t => t.id === "pressure-ceiling-events")).toBe(true);
  for (const tip of tips) {
    const copy = `${tip.title} ${tip.body} ${tip.evidence}`;
    expect(copy).not.toMatch(/\b(increase|decrease|raise|lower)\b/i);
    expect(copy).not.toMatch(/\b\d+(?:\.\d+)?\s*(?:hPa|cm\s*H2O|cmH₂O)\b/i);
    expect(copy).not.toMatch(/(?:set|target|adjust|change)\s+(?:your\s+)?pressure\s+(?:to|at)\s*\d/i);
    expect(tip.evidence).toBeTruthy();
    expect(tip.evidenceDates.length).toBeGreaterThan(0);
    expect(tip.evidenceDates.length).toBeLessThanOrEqual(5);
  }
});

it("keeps event and leak confounders uncertain while mask-off epochs remain no-data", () => {
  const date = mockNights().find(n => (n.largeLeakMinutes ?? 0) > 20)!.date;
  const detail = mockNightDetail(date)!;
  expect(detail.stages!.estimatorVersion).toBe(3);
  expect(detail.stages!.confidence).toEqual({ sleepWake: "moderate", rem: "low", deep: "veryLow" });
  expect(detail.stages!.minutes.uncertain).toBeGreaterThan(0);
  for (const event of detail.events.filter(e => ["obstructiveApnea", "centralApnea", "hypopnea"].includes(e.kind))) {
    const epoch = Math.floor((wallSeconds(event.time) - wallSeconds(detail.stages!.start)) / 30);
    if (detail.stages!.stages[epoch] !== "noData") expect(detail.stages!.stages[epoch]).toBe("uncertain");
  }
});

it("uses the backend directional P90 boundary rule and rejects out-of-range reassurance", () => {
  const base = mockPressureObservation(mockNights()[20]!.date)!;
  const above = { ...base, item: { ...base.item, ahi: 1, excessLeakP95: 0, pressureP90: base.settings.maxPressure + 2 } };
  const below = { ...base, item: { ...base.item, ahi: 1, excessLeakP95: 0, pressureP90: base.settings.minPressure - 2 } };
  expect(pressureSummary([above, below])).toMatchObject({ nightsNearMax: 1, nightsNearMin: 1 });
  expect(pressureTips([above, below]).some(t => t.id === "pressure-ok")).toBe(false);
});

it("keeps reported P90 aligned with the generated pressure trace", () => {
  for (const index of [20, 55, 60, 82, 103, 118]) {
    const detail = mockNightDetail(addDays(MOCK_FIRST_DATE, index))!;
    const values = detail.series!.pressure.filter((v): v is number => v != null).sort((a, b) => a - b);
    const actual = values[Math.floor((values.length - 1) * 0.9)]!;
    expect(Math.abs(actual - detail.pressureP90!)).toBeLessThanOrEqual(0.101);
  }
});

function synthetic(date = "2026-01-01"): PressureObservation {
  const base = mockPressureObservation(mockNights()[20]!.date)!;
  return { ...base, eventsAtMin: 0, shareAtMaxPct: 0, item: { ...base.item, date, usageMinutes: 420, ahi: 1, pressureP90: 6, excessLeakP95: 0, largeLeakMinutes: 0, eventCounts: { obstructive: 1, central: 0, hypopnea: 0 } } };
}
const periodNights = (count: number) => Array.from({ length: count }, (_, i) => synthetic(addDays("2026-01-01", i)));

it("uses count-based evidence for one and two supporting nights", () => {
  for (const count of [1, 2]) {
    const nights = periodNights(7);
    for (const n of nights.slice(0, count)) n.centralIndex = 6;
    const evidence = pressureTips(nights, true).find(t => t.id === "central-with-pressure")!.evidence!;
    expect(evidence).toContain(`on ${count} ${count === 1 ? "night." : "nights."}`);
  }
});

it("requires central event burden and a high-versus-low leak comparison in long periods", () => {
  const nights = periodNights(120);
  nights[0]!.item.eventCounts = { central: 3, obstructive: 1, hypopnea: 0 };
  nights[0]!.item.pressureP90 = nights[0]!.settings.maxPressure;
  nights[0]!.item.excessLeakP95 = 25;
  nights[0]!.shareAtMaxPct = 20;
  for (const n of nights.slice(1)) {
    n.item.pressureP90 = n.settings.minPressure;
    n.item.excessLeakP95 = 20;
  }
  expect(pressureTips(nights, true).map(t => t.id)).not.toContain("central-with-pressure");
  expect(pressureTips(nights, true).map(t => t.id)).not.toContain("leak-at-high-pressure");
  nights[0]!.centralIndex = 6;
  expect(pressureTips(nights, true).map(t => t.id)).toContain("central-with-pressure");
  nights[0]!.item.excessLeakP95 = 30;
  expect(pressureTips(nights, true).map(t => t.id)).toContain("leak-at-high-pressure");
});

it("suppresses info floor events for low use and keeps watch burden without reassurance", () => {
  const nights = periodNights(7);
  for (const n of nights) {
    n.item.pressureP90 = n.settings.minPressure;
    n.item.eventCounts.obstructive = 7;
    n.eventsAtMin = 7;
  }
  let ids = pressureTips(nights, true).map(t => t.id);
  expect(ids).toContain("pressure-low-use");
  expect(ids).not.toContain("pressure-floor-events");
  for (const n of nights.slice(0, 3)) {
    n.item.eventCounts.obstructive = 21;
    n.eventsAtMin = 21;
  }
  ids = pressureTips(nights, true).map(t => t.id);
  expect(ids).toContain("pressure-floor-events");
  expect(ids).not.toContain("pressure-low-use");
});
