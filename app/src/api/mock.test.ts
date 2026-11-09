import { describe, expect, it } from "vitest";
import { wallSeconds } from "../lib/dates";
import { createMockApi } from "./mock";
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
        expect(n.ahi).toBeLessThanOrEqual(2.6);
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
