import { describe, expect, it } from "vitest";
import { wallSeconds } from "../lib/dates";
import { computePeriodSummary, computeTrends, createMockApi } from "./mock";
import type { ImportProgress, ImportResult } from "./types";

function waitFinished(api: ReturnType<typeof createMockApi>): Promise<{ result: ImportResult; progress: ImportProgress[] }> {
  const progress: ImportProgress[] = [];
  return new Promise((resolve) => {
    void api.onImportProgress((p) => progress.push(p));
    void api.onImportFinished((r) => resolve({ result: r, progress }));
  });
}

describe("mock api", () => {
  it("starts empty, imports sample data and reports progress", async () => {
    const api = createMockApi({ tickMs: 0 });
    expect((await api.appInfo()).nightCount).toBe(0);
    expect(await api.listNights()).toEqual([]);
    const done = waitFinished(api);
    await api.loadSampleData();
    const { result, progress } = await done;
    expect(result.status).toBe("ok");
    expect(result.nightsAdded).toBeGreaterThan(100);
    expect(progress.map((p) => p.phase)).toEqual(expect.arrayContaining(["scanning", "summaries", "nights", "done"]));
    const info = await api.appInfo();
    expect(info.nightCount).toBe(result.nightsAdded);
    expect(info.device?.serialMasked).toMatch(/^•••..$/);
    expect(info.device?.pressureUnit).toBe("hPa");
  });

  it("has about 120 plausible nights", async () => {
    const api = createMockApi({ preloaded: true });
    const nights = await api.listNights();
    expect(nights.length).toBeGreaterThanOrEqual(115);
    expect(nights.length).toBeLessThanOrEqual(125);
    const sorted = [...nights].sort((a, b) => (a.date < b.date ? -1 : 1));
    expect(nights).toEqual(sorted);
    for (const n of nights) {
      if (n.ahi != null) {
        expect(n.ahi).toBeGreaterThanOrEqual(0.3);
        expect(n.ahi).toBeLessThanOrEqual(9.1);
      }
      expect(n.pressureP90!).toBeGreaterThanOrEqual(n.pressureMean!);
    }
    expect(nights.filter((n) => n.settingsChanged)).toHaveLength(1);
    expect(nights.filter((n) => (n.largeLeakMinutes ?? 0) > 20).length).toBeGreaterThanOrEqual(3);
    expect(nights.some((n) => n.ahi == null && n.pressureSource === "computed")).toBe(true);
    const history = await api.settingsHistory();
    expect(history.filter((h) => h.changed.length > 0)).toHaveLength(1);
  });

  it("night detail agrees with the summary and has aligned 1 Hz series", async () => {
    const api = createMockApi({ preloaded: true });
    const nights = await api.listNights();
    const last = nights[nights.length - 1]!;
    const d = await api.nightDetail(last.date);
    const s = d.series!;
    const expectedLen = wallSeconds(d.end!) - wallSeconds(d.start!) + 1;
    expect(s.pressure.length).toBe(expectedLen);
    for (const k of ["excessLeak", "totalLeak", "flowAmplitude", "snore"] as const) expect(s[k].length).toBe(expectedLen);
    const kinds = (k: string) => d.events.filter((e) => e.kind === k).length;
    expect(kinds("obstructiveApnea")).toBe(d.eventCounts.obstructive);
    expect(kinds("centralApnea")).toBe(d.eventCounts.central);
    expect(kinds("hypopnea")).toBe(d.eventCounts.hypopnea);
    const used = d.sessions.reduce((a, x) => a + x.minutes, 0);
    expect(used).toBe(d.usageMinutes);
    const min = d.settings!.minPressure, max = d.settings!.maxPressure;
    for (const p of s.pressure) if (p != null) { expect(p).toBeGreaterThanOrEqual(min); expect(p).toBeLessThanOrEqual(max); }
  });

  it("computes trends with the 4-hour valid-night rule", async () => {
    const api = createMockApi({ preloaded: true });
    const nights = await api.listNights();
    const to = nights[nights.length - 1]!.date;
    const t = await api.trends({ from: "2026-10-10", to });
    const inRange = nights.filter((n) => n.date >= "2026-10-10" && n.date <= to);
    expect(t.points).toHaveLength(inRange.length);
    expect(t.validNights).toBe(inRange.filter((n) => n.usageMinutes > 240).length);
    expect(t.compliancePct).toBeCloseTo((t.validNights / t.daysInRange) * 100, 5);
    expect(t.daysInRange).toBe(30);
  });

  it("can cancel an import and delete all data", async () => {
    const api = createMockApi({ tickMs: 5 });
    const done = waitFinished(api);
    await api.importCard("/Volumes/NO NAME");
    await api.cancelImport();
    expect((await done).result.status).toBe("cancelled");
    expect((await api.appInfo()).nightCount).toBe(0);

    const api2 = createMockApi({ preloaded: true });
    await api2.deleteAllData();
    expect((await api2.appInfo()).nightCount).toBe(0);
    await expect(api2.nightDetail("2026-11-08")).rejects.toThrow();
  });

  it("reports an unsupported folder as an error", async () => {
    const api = createMockApi({ tickMs: 0 });
    const done = waitFinished(api);
    await api.importCard("/Users/you/Documents");
    expect((await done).result.status).toBe("error");
  });
});


describe("v2 periods", () => {
  it("uses inclusive fixed-length ranges and fills missing calendar days", async () => {
    const api = createMockApi({ preloaded: true });
    const nights = await api.listNights();
    for (const [period, length] of [["week", 7], ["month", 30], ["6months", 182], ["year", 365]] as const) {
      const p = await api.periodSummary(period);
      expect(p.daysInRange).toBe(length);
      expect(p.scoreSeries).toHaveLength(length);
      expect(p.scoreSeries[0]!.date).toBe(p.from);
      expect(p.scoreSeries[length - 1]!.date).toBe(p.to);
      const ns = nights.filter(n => n.date >= p.from && n.date <= p.to);
      expect(p.nights).toBe(ns.length);
      expect(p.avgUsageMinutes).toBeCloseTo(ns.reduce((s,n) => s + n.usageMinutes, 0) / ns.length);
      expect(p.compliancePct).toBeCloseTo(ns.filter(n => n.usageMinutes > 240).length / length * 100);
      expect(p.bestNight!.sleepScore).toBe(Math.max(...ns.map(n => n.sleepScore!)));
      expect(p.worstNight!.sleepScore).toBe(Math.min(...ns.map(n => n.sleepScore!)));
      expect(p.tips.length).toBeLessThanOrEqual(5);
    }
    const p = await api.periodSummary("month", "2026-08-25");
    expect(p.scoreSeries.some(n => n.sleepScore == null && n.usageMinutes === 0)).toBe(true);
    const previous = await api.periodSummary("month", "2026-07-26");
    expect(p.previous.avgSleepScore).toBe(previous.avgSleepScore);
    expect(p.previous.compliancePct).toBe(previous.compliancePct);
  });

  it("keeps no-data averages null and averages only nights with a stage estimate", async () => {
    const api = createMockApi({ preloaded: true });
    const empty = await api.periodSummary("week", "2020-01-01");
    expect(empty.nights).toBe(0);
    expect(empty.avgUsageMinutes).toBeNull();
    expect(empty.avgStageMinutes).toBeNull();
    expect(empty.bestNight).toBeNull();
    expect(empty.previous).toEqual({ avgUsageMinutes: null, avgAhi: null, avgSleepScore: null, compliancePct: null });
    const ns = (await api.listNights()).slice(-2);
    const p = computePeriodSummary(ns, "week", ns[1]!.date, d => d === ns[0]!.date ? { wake: 10, light: 200, deep: 40, rem: 70 } : null);
    expect(p.avgStageMinutes).toEqual({ wake: 10, light: 200, deep: 40, rem: 70 });
  });

  it("has varied scores and plausible staged epochs aligned to mask-off gaps", async () => {
    const api = createMockApi({ preloaded: true });
    const ns = await api.listNights();
    const scores = ns.flatMap(n => n.sleepScore == null ? [] : [n.sleepScore]);
    expect(Math.min(...scores)).toBeGreaterThanOrEqual(55);
    expect(Math.min(...scores)).toBeLessThanOrEqual(65);
    expect(Math.max(...scores)).toBeLessThanOrEqual(92);
    expect(Math.max(...scores)).toBeGreaterThanOrEqual(88);
    const interrupted = ns.find(n => n.interruptions > 0 && n.ahi != null)!;
    const d = await api.nightDetail(interrupted.date);
    expect(d.score!.total).toBe(d.sleepScore);
    expect(d.score!.components).toHaveLength(5);
    expect(d.stages!.epochSeconds).toBe(30);
    expect(d.stages!.stages[0]).toBe("wake");
    expect(d.stages!.stages).toContain("noData");
    expect(Object.values(d.stages!.minutes).reduce((s,m) => s + m, 0)).toBe(d.usageMinutes);
    for (const [i, stage] of d.stages!.stages.entries()) {
      const t = wallSeconds(d.stages!.start) + i * 30;
      const on = d.sessions.some(s => t >= wallSeconds(s.start) && t < wallSeconds(s.end));
      expect(stage === "noData").toBe(!on);
    }
  });
});


it("period math excludes zero use and treats exactly four hours as invalid", async () => {
  const api = createMockApi({ preloaded: true });
  const base = (await api.listNights())[10]!;
  const fixtures = [
    { ...base, date: "2026-10-25", usageMinutes: 999, ahi: 99, sleepScore: 1 },
    { ...base, date: "2026-10-26", usageMinutes: 120, ahi: null, sleepScore: 50 },
    { ...base, date: "2026-11-01", usageMinutes: 300, ahi: 4, sleepScore: 70 },
    { ...base, date: "2026-11-02", usageMinutes: 0, ahi: 99, sleepScore: 1 },
    { ...base, date: "2026-11-03", usageMinutes: 240, ahi: null, sleepScore: 60 },
    { ...base, date: "2026-11-08", usageMinutes: 241, ahi: 2, sleepScore: 80 },
    { ...base, date: "2026-11-09", usageMinutes: 888, ahi: 88, sleepScore: 1 }
  ];
  const p = computePeriodSummary(fixtures, "week", "2026-11-08", () => null);
  expect(p.from).toBe("2026-11-02");
  expect(p.nights).toBe(2);
  expect(p.validNights).toBe(1);
  expect(p.avgUsageMinutes).toBe(240.5);
  expect(p.avgAhi).toBe(2);
  expect(p.avgSleepScore).toBe(70);
  expect(p.compliancePct).toBeCloseTo(100 / 7);
  expect(p.previous).toEqual({ avgUsageMinutes: 210, avgAhi: 4, avgSleepScore: 60, compliancePct: (1 / 7) * 100 });
});


it("only marks mode and pressure prescription changes", async () => {
  const api = createMockApi({ preloaded: true });
  const ns = (await api.listNights()).slice(-3).map(n => ({ ...n, settingsChanged: true }));
  const settings = (await api.settingsHistory())[0]!.settings;
  const result = computeTrends(ns, undefined, [
    { date: ns[0]!.date, time: null, settings, changed: ["humidifierLevel", "rampMinutes"] },
    { date: ns[1]!.date, time: null, settings, changed: ["minPressure"] },
    { date: ns[2]!.date, time: null, settings, changed: ["comfortLevel"] }
  ]);
  expect(result.points.map(p => p.prescriptionChanged)).toEqual([false, true, false]);
});
