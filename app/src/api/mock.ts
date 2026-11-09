import { nightTips } from "./mockV2";
import { addDays, daysInclusive } from "../lib/dates";
import {
  MOCK_DEVICE, MOCK_FIRST_DATE, mockMonths, mockNightDetail, mockNights, mockSettingsHistory, mockStageMinutes
} from "./mockData";
import type {
  AppInfo, CardCandidate, DateRange, ImportProgress, ImportResult, ImportStarted, NightListItem,
  SleepLensApi, Trends, Period, PeriodSummary, StageMinutes, Tip, SettingsChange
} from "./types";

export interface MockOptions {
  /** Start with data already imported (skips the first-launch screen). */
  preloaded?: boolean;
  /** Remember "imported" across page reloads in this browser tab (sessionStorage). */
  persist?: boolean;
  /** Milliseconds between simulated progress steps (0 in tests). */
  tickMs?: number;
}

const STORE_KEY = "sleeplens.mock.imported";
export const MOCK_CARD_PATH = "/Volumes/NO NAME";

function readPersisted(): boolean {
  try { return sessionStorage.getItem(STORE_KEY) === "1"; } catch { return false; }
}
function writePersisted(v: boolean) {
  try { if (v) sessionStorage.setItem(STORE_KEY, "1"); else sessionStorage.removeItem(STORE_KEY); } catch { /* storage unavailable */ }
}

function inRange(d: string, range?: DateRange) {
  return (!range?.from || d >= range.from) && (!range?.to || d <= range.to);
}

const mean = (a: number[]) => (a.length ? a.reduce((s, v) => s + v, 0) / a.length : null);

export function prescriptionDates(history: SettingsChange[]): Set<string> {
  const fields = new Set(["mode", "modeLabel", "startPressure", "minPressure", "maxPressure", "fixedPressure"]);
  return new Set(history.filter(h => h.changed.some(key => fields.has(key))).map(h => h.date));
}

export function computeTrends(all: NightListItem[], range?: DateRange, history = mockSettingsHistory()): Trends {
  const prescriptions = prescriptionDates(history);
  const first = all[0]?.date ?? range?.from ?? "1970-01-01";
  const last = all[all.length - 1]?.date ?? range?.to ?? first;
  const from = range?.from ?? first;
  const to = range?.to ?? last;
  const ns = all.filter((n) => inRange(n.date, { from, to }));
  const used = ns.filter((n) => n.usageMinutes > 0);
  const valid = ns.filter((n) => n.usageMinutes > 240).length;
  const days = Math.max(1, daysInclusive(from, to));
  const ahis = used.map((n) => n.ahi).filter((v): v is number => v != null);
  const p90s = used.map((n) => n.pressureP90).filter((v): v is number => v != null);
  const leaks = used.map((n) => n.excessLeakMedian).filter((v): v is number => v != null);
  return {
    from, to,
    nights: used.length,
    daysInRange: days,
    validNights: valid,
    compliancePct: (valid / days) * 100,
    avgUsageMinutes: mean(used.map((n) => n.usageMinutes)) ?? 0,
    avgAhi: mean(ahis),
    avgPressureP90: mean(p90s),
    avgExcessLeak: mean(leaks),
    avgSleepScore: mean(used.flatMap(n => n.sleepScore == null ? [] : [n.sleepScore])),
    avgExcessLeakP95: mean(used.flatMap(n => n.excessLeakP95 == null ? [] : [n.excessLeakP95])),
    points: ns.map((n) => ({
      date: n.date, usageMinutes: n.usageMinutes, ahi: n.ahi, pressureP90: n.pressureP90,
      excessLeakMedian: n.excessLeakMedian, settingsChanged: n.settingsChanged,
      sleepScore: n.sleepScore, excessLeakP95: n.excessLeakP95, prescriptionChanged: prescriptions.has(n.date)
    }))
  };
}

export const PERIOD_DAYS: Record<Period, number> = { week: 7, month: 30, "6months": 182, year: 365 };

export function computePeriodSummary(
  all: NightListItem[], period: Period, end = all[all.length - 1]?.date ?? "1970-01-01",
  stageMinutes: (date: string) => StageMinutes | null = mockStageMinutes,
  history = mockSettingsHistory()
): PeriodSummary {
  const days = PERIOD_DAYS[period];
  const from = addDays(end, 1 - days);
  const current = computeTrends(all, { from, to: end }, history);
  const before = computeTrends(all, { from: addDays(from, -days), to: addDays(from, -1) }, history);
  const ns = all.filter(n => inRange(n.date, { from, to: end }) && n.usageMinutes > 0);
  const stages = ns.map(n => stageMinutes(n.date)).filter((v): v is StageMinutes => v != null);
  const avgStageMinutes = stages.length ? Object.fromEntries(
    (["wake", "light", "deep", "rem"] as const).map(key => [key, mean(stages.map(s => s[key]))!])
  ) as unknown as StageMinutes : null;
  const scored = ns.filter((n): n is NightListItem & { sleepScore: number } => n.sleepScore != null)
    .sort((a, b) => a.sleepScore - b.sleepScore || a.date.localeCompare(b.date));
  const night = (n: typeof scored[number] | undefined) => n ? { date: n.date, sleepScore: n.sleepScore } : null;
  const tips: Tip[] = [];
  const add = (id: string, severity: Tip["severity"], title: string, body: string, learnSlug: string) => tips.push({ id, severity, title, body, learnSlug });
  // Clinical and mask-fit tips keep their priority; the period-specific guides follow.
  for (const n of ns) for (const tip of nightTips(n, all.filter(p => p.date <= n.date && p.date >= addDays(n.date, -6)))) {
    if (!["all-good", "prescription-changed"].includes(tip.id)) {
      const existing = tips.findIndex(t => t.id === tip.id);
      if (existing < 0) tips.push(tip);
      else if (tip.severity === "talk") tips[existing] = tip;
    }
  }
  tips.sort((a, b) => ["talk", "watch", "info", "good"].indexOf(a.severity) - ["talk", "watch", "info", "good"].indexOf(b.severity));
  if (current.compliancePct < 70) add("compliance-watch", "watch", "Build a steady therapy routine", "Fewer than 70% of days had more than four hours of use. Aim to use your mask whenever you sleep.", "compliance");
  else if (current.compliancePct >= 90) add("compliance-good", "good", "A steady therapy routine", "At least 90% of days had more than four hours of use. Keep using therapy whenever you sleep.", "compliance");
  if (current.avgSleepScore != null && before.avgSleepScore != null) {
    const delta = current.avgSleepScore - before.avgSleepScore;
    if (delta >= 5) add("score-up", "good", "Your average score is up", "Your average estimated sleep score rose by at least five points compared with the previous period.", "sleepScore");
    else if (delta <= -5) add("score-down", "info", "Your average score is lower", "Your average estimated sleep score fell by at least five points. Review the breakdown and look for changes in your routine.", "sleepScore");
  }
  if (current.points.some(n => n.prescriptionChanged)) add("prescription-changed", "info", "Compare before and after", "Your prescription changed during this period. Compare nights before and after with your clinician.", "pressure");
  if (!tips.length) add("all-good", "good", "Keep your routine going", "Your recorded therapy measures look steady. Keep using your mask whenever you sleep.", "sleepScore");
  const byDate = new Map(ns.map(n => [n.date, n]));
  return {
    period, from, to: end, daysInRange: days, nights: current.nights, validNights: current.validNights,
    compliancePct: current.compliancePct, avgUsageMinutes: current.nights ? current.avgUsageMinutes : null,
    avgAhi: current.avgAhi, avgSleepScore: current.avgSleepScore, avgExcessLeakP95: current.avgExcessLeakP95,
    avgPressureP90: current.avgPressureP90, avgStageMinutes,
    previous: { avgUsageMinutes: before.nights ? before.avgUsageMinutes : null, avgAhi: before.avgAhi,
      avgSleepScore: before.avgSleepScore, compliancePct: before.nights ? before.compliancePct : null },
    bestNight: night(scored[scored.length - 1]), worstNight: night(scored[0]),
    scoreSeries: Array.from({ length: days }, (_, i) => {
      const date = addDays(from, i), n = byDate.get(date);
      return { date, sleepScore: n?.sleepScore ?? null, usageMinutes: n?.usageMinutes ?? 0 };
    }),
    tips: tips.sort((a, b) => ["talk", "watch", "info", "good"].indexOf(a.severity) - ["talk", "watch", "info", "good"].indexOf(b.severity)).slice(0, 5)
  };
}

export function createMockApi(opts: MockOptions = {}): SleepLensApi {
  const tick = opts.tickMs ?? 45;
  let imported = opts.preloaded ?? (opts.persist ? readPersisted() : false);
  let importCount = imported ? 1 : 0;
  let lastImportAt: string | null = imported ? "2026-11-09T07:58:00" : null;
  let running: { id: string; cancelled: boolean } | null = null;
  let seq = 0;
  const progressHandlers = new Set<(p: ImportProgress) => void>();
  const finishedHandlers = new Set<(r: ImportResult) => void>();

  const nights = () => (imported ? mockNights() : []);
  const emit = (p: ImportProgress) => progressHandlers.forEach((h) => h(p));
  const finish = (r: ImportResult) => finishedHandlers.forEach((h) => h(r));
  const wait = (ms: number) => new Promise<void>((res) => setTimeout(res, ms));

  async function runImport(id: string, kind: "card" | "sample") {
    const all = mockNights();
    const months = mockMonths();
    const job = running!;
    const cancelled = () => job.cancelled;
    const stop = () => {
      emit({ phase: "cancelled", done: 0, total: all.length, message: "Import cancelled." });
      finish({ importId: id, status: "cancelled", nightsAdded: 0, nightsUpdated: 0, nightsSkipped: 0, nightsFailed: [] });
      running = null;
    };
    for (let m = 0; m <= months.length; m++) {
      if (cancelled()) return stop();
      emit({ phase: "scanning", done: m, total: months.length, message: months[m] ? `Looking in ${months[m]}` : undefined });
      await wait(tick * 2);
    }
    emit({ phase: "summaries", done: 0, total: 1, message: "Reading the machine's summary log" });
    await wait(tick * 4);
    emit({ phase: "summaries", done: 1, total: 1 });
    const again = importCount > 0;
    const step = 3;
    for (let i = 0; i < all.length; i += step) {
      if (cancelled()) return stop();
      emit({ phase: "nights", done: Math.min(i + step, all.length), total: all.length, currentDate: all[Math.min(i + step, all.length) - 1]!.date });
      await wait(again ? Math.max(1, tick / 4) : tick);
    }
    if (cancelled()) return stop();
    imported = true;
    importCount++;
    lastImportAt = new Date().toISOString().slice(0, 19);
    if (opts.persist) writePersisted(true);
    const failed = kind === "card" && !again
      ? [{ date: addDays(MOCK_FIRST_DATE, 17), reason: "The night file ends early (the card may have been removed while the machine was writing)." }]
      : [];
    emit({ phase: "done", done: all.length, total: all.length });
    finish({
      importId: id,
      status: "ok",
      nightsAdded: again ? 0 : all.length,
      nightsUpdated: again ? 1 : 0,
      nightsSkipped: again ? all.length - 1 : 0,
      nightsFailed: failed
    });
    running = null;
  }

  function start(kind: "card" | "sample", path?: string): Promise<ImportStarted> {
    if (running) return Promise.reject(new Error("An import is already running."));
    const id = `mock-${++seq}`;
    if (kind === "card" && path && !path.startsWith("/Volumes/")) {
      // Simulate an unsupported folder.
      setTimeout(() => {
        emit({ phase: "error", done: 0, total: 0, message: "No supported CPAP data found in this folder." });
        finish({
          importId: id, status: "error", nightsAdded: 0, nightsUpdated: 0, nightsSkipped: 0, nightsFailed: [],
          message: "No supported CPAP data found. Found folders: Documents, Photos."
        });
      }, tick * 3);
      return Promise.resolve({ importId: id });
    }
    running = { id, cancelled: false };
    void runImport(id, kind);
    return Promise.resolve({ importId: id });
  }

  return {
    kind: "mock",
    async appInfo(): Promise<AppInfo> {
      const ns = nights();
      return {
        version: "0.2.0 (mock data)",
        needsReread: 0,
        dataDir: "/Users/you/Library/Application Support/SleepLens",
        dbSizeBytes: imported ? 96_000 + ns.length * 712_000 : 49_152,
        device: imported ? MOCK_DEVICE : null,
        nightCount: ns.length,
        firstDate: ns[0]?.date ?? null,
        lastDate: ns[ns.length - 1]?.date ?? null,
        lastImportAt
      };
    },
    async findCards(): Promise<CardCandidate[]> {
      return [{ path: MOCK_CARD_PATH, label: "NO NAME", family: "AS2X", nights: mockNights().length, months: mockMonths() }];
    },
    importCard: (path) => start("card", path),
    async cancelImport() {
      if (running) running.cancelled = true;
      return null;
    },
    async listNights(range) {
      return nights().filter((n) => inRange(n.date, range));
    },
    async nightDetail(date) {
      const d = imported ? mockNightDetail(date) : null;
      if (!d) throw new Error(`No night recorded on ${date}`);
      return d;
    },
    async trends(range) {
      return computeTrends(nights(), range);
    },
    async periodSummary(period, end) {
      return computePeriodSummary(nights(), period, end);
    },
    async settingsHistory() {
      return imported ? mockSettingsHistory() : [];
    },
    async deleteAllData() {
      imported = false;
      importCount = 0;
      lastImportAt = null;
      if (opts.persist) writePersisted(false);
      return null;
    },
    loadSampleData: () => start("sample"),
    async onImportProgress(h) { progressHandlers.add(h); return () => progressHandlers.delete(h); },
    async onImportFinished(h) { finishedHandlers.add(h); return () => finishedHandlers.delete(h); },
    async pickFolder() {
      await new Promise((res) => setTimeout(res, 150));
      return MOCK_CARD_PATH;
    },
    async onDragDrop() {
      return null; // the browser cannot give folder paths; the UI uses HTML drag events instead
    }
  };
}
