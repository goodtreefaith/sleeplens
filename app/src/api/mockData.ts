// Synthetic data for a fictional Aeonmed AS100A. Deterministic (seeded), so screenshots and tests are stable.
// Nothing here comes from a real card.
import { pressureTips, type PressureObservation } from "./mockPressure";
import { computeMockScore, estimateMockStages, nightTips } from "./mockV2";
import { addDays, daysInclusive, secondsToWall, wallSeconds } from "../lib/dates";
import type {
  Device, EventCounts, NightDetail, NightListItem, Settings, SettingsChange, SleepScore, SleepStageEstimate, TherapyEvent
} from "./types";

export const MOCK_LAST_DATE = "2026-11-08";
export const MOCK_DAYS = 124;
export const MOCK_FIRST_DATE = addDays(MOCK_LAST_DATE, -(MOCK_DAYS - 1));
const SKIPPED = new Set([17, 44, 45, 88]);          // days with no therapy (gaps in the calendar)
const LEAK_NIGHTS = new Set([23, 57, 96, 118]);     // nights with a real large leak
const SHORT_NIGHTS = new Set([31]);                 // one short night
const NO_AHI_BEFORE = 8;                            // the .bkp summary no longer covers the oldest nights
export const SETTINGS_CHANGE_INDEX = 70;

export const MOCK_DEVICE: Device = {
  brand: "Aeonmed",
  model: "AS100A",
  family: "AS2X",
  firmware: "1.01.47.01",
  hardware: "2.0",
  serialMasked: "•••42",
  pressureUnit: "hPa"
};

const BASE_SETTINGS: Settings = {
  mode: 1, modeLabel: "Auto CPAP",
  startPressure: 4, minPressure: 4, maxPressure: 8, fixedPressure: 6,
  rampMinutes: 15, humidifierLevel: 3, comfortLevel: 2,
  autoOn: true, autoOff: true, leakAlert: false,
  pressureUnit: "hPa"
};
const CHANGED_SETTINGS: Settings = { ...BASE_SETTINGS, startPressure: 5, minPressure: 5, maxPressure: 9 };

/** Small fast seeded PRNG (mulberry32). */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function seedOf(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
  return h >>> 0;
}

const round1 = (v: number) => Math.round(v * 10) / 10;
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

interface NightPlan {
  item: NightListItem;
  index: number;
  sessions: { start: string; end: string; minutes: number }[];
  settings: Settings;
  score: SleepScore | null;
  stages: SleepStageEstimate;
  therapyEvents: TherapyEvent[];
  leakBlocks: [number, number][];
}

function planNight(index: number): NightPlan | null {
  if (SKIPPED.has(index)) return null;
  const date = addDays(MOCK_FIRST_DATE, index);
  const r = rng(seedOf("night" + date));
  const settings = index >= SETTINGS_CHANGE_INDEX ? CHANGED_SETTINGS : BASE_SETTINGS;

  const usage = SHORT_NIGHTS.has(index) ? 205 + Math.round(r() * 30) : Math.round(300 + r() * 240);
  const startMin = 22 * 60 + Math.round(r() * 100);               // 22:00 - 23:40
  const t0 = wallSeconds(`${date}T00:00:00`) + startMin * 60;
  const twoSessions = r() < 0.3;
  const sessions: NightPlan["sessions"] = [];
  if (twoSessions) {
    const first = Math.round(usage * (0.3 + r() * 0.3));
    const gap = 10 + Math.round(r() * 30);
    sessions.push({ start: secondsToWall(t0), end: secondsToWall(t0 + first * 60), minutes: first });
    const s2 = t0 + (first + gap) * 60;
    sessions.push({ start: secondsToWall(s2), end: secondsToWall(s2 + (usage - first) * 60), minutes: usage - first });
  } else {
    sessions.push({ start: secondsToWall(t0), end: secondsToWall(t0 + usage * 60), minutes: usage });
  }

  const hours = usage / 60;
  const ceilingWeek = index >= 100 && index <= 106;
  const ahiTarget = clamp(2.3 + (r() - 0.5) * 3.4 + (LEAK_NIGHTS.has(index) || ceilingWeek || index === 62 ? 5 : 0), 0.3, 9);
  const total = Math.max(Math.ceil(0.3 * hours), Math.round(ahiTarget * hours));
  const obstructive = Math.round(total * (0.35 + r() * 0.2));
  const central = index === 62 ? Math.round(total * 0.65) : Math.round((total - obstructive) * (0.25 + r() * 0.2));
  const counts: EventCounts = { obstructive: Math.min(obstructive, total - central), central, hypopnea: Math.max(0, total - Math.min(obstructive, total - central) - central) };
  const hasAhi = index >= NO_AHI_BEFORE;

  const p90 = ceilingWeek || index === 60 || LEAK_NIGHTS.has(index) ? settings.maxPressure - 0.2 : (index >= 50 && index <= 56 || index >= 80 && index <= 82) ? settings.minPressure + 0.3 : round1(clamp(settings.minPressure + 1.2 + r() * 1.8, settings.minPressure + 1, settings.maxPressure - 0.7));
  const pMean = round1(p90 - 0.15 - r() * 0.1);
  const leakNight = LEAK_NIGHTS.has(index);

  const item: NightListItem = {
    date,
    start: sessions[0]!.start,
    end: sessions[sessions.length - 1]!.end,
    usageMinutes: usage,
    usageSource: "machine",
    ahi: hasAhi ? round1(total / hours) : null,
    pressureMean: pMean,
    pressureP90: p90,
    pressureSource: hasAhi ? "machine" : "computed",
    excessLeakMedian: 0,
    excessLeakP95: leakNight ? round1(30 + r() * 12) : round1(8 + r() * 14),
    sleepScore: null,
    interruptions: sessions.length - 1,
    largeLeakMinutes: hasAhi ? (leakNight ? Math.round(25 + r() * 55) : Math.round(r() * 3)) : null,
    eventCounts: hasAhi ? counts : { obstructive: 0, central: 0, hypopnea: 0 },
    settingsChanged: index === SETTINGS_CHANGE_INDEX
  };
  const therapyEvents: TherapyEvent[] = [];
  const onOffsets = sessions.flatMap(session => Array.from({ length: session.minutes * 2 }, (_, i) => wallSeconds(session.start) - t0 + i * 30));
  for (const [kind, count, code] of [["obstructiveApnea", item.eventCounts.obstructive, 0xe1], ["centralApnea", item.eventCounts.central, 0xe2], ["hypopnea", item.eventCounts.hypopnea, 0xe3]] as const) {
    for (let j = 0; j < count; j++) {
      const at = onOffsets[Math.floor(r() * onOffsets.length)] ?? 0;
      therapyEvents.push({ time: secondsToWall(t0 + at), kind, code, value: 12 + Math.round(r() * 18), pressure: null });
    }
  }
  const leakBlocks: [number, number][] = leakNight ? [[onOffsets[Math.floor(onOffsets.length * 0.45)] ?? 0, (onOffsets[Math.floor(onOffsets.length * 0.45)] ?? 0) + (item.largeLeakMinutes ?? 20) * 60]] : [];
  const stages = estimateMockStages(item, sessions, index, therapyEvents, leakBlocks);
  const score = computeMockScore(item, stages);
  item.sleepScore = score?.total ?? null;
  return { item, index, sessions, settings, stages, score, therapyEvents, leakBlocks };
}

let plansCache: NightPlan[] | null = null;
function plans(): NightPlan[] {
  if (!plansCache) {
    plansCache = [];
    for (let i = 0; i < MOCK_DAYS; i++) { const p = planNight(i); if (p) plansCache.push(p); }
  }
  return plansCache;
}

export function mockStageMinutes(date: string) {
  return plans().find(p => p.item.date === date)?.stages.minutes ?? null;
}

export function mockNights(): NightListItem[] {
  return plans().map((p) => p.item);
}

export function mockSettingsHistory(): SettingsChange[] {
  const ns = plans();
  const changeNight = ns.find((p) => p.index === SETTINGS_CHANGE_INDEX)!;
  return [
    { date: ns[0]!.item.date, time: null, settings: BASE_SETTINGS, changed: [] },
    {
      date: changeNight.item.date,
      time: `${changeNight.item.date}T19:42:10`,
      settings: CHANGED_SETTINGS,
      changed: ["startPressure", "minPressure", "maxPressure"]
    }
  ];
}

export function mockMonths(): string[] {
  const months = new Set(plans().map((p) => p.item.date.slice(0, 7)));
  return [...months];
}

const detailCache = new Map<string, NightDetail>();

/** Builds a full night with 1 Hz series and events that agree with the night's summary. */
export function mockNightDetail(date: string): NightDetail | null {
  const cached = detailCache.get(date);
  if (cached) return cached;
  const plan = plans().find((p) => p.item.date === date);
  if (!plan) return null;
  const { item, sessions, settings } = plan;
  const r = rng(seedOf("detail" + date));

  const t0 = wallSeconds(item.start!);
  const t1 = wallSeconds(item.end!);
  const n = Math.round(t1 - t0) + 1;
  const on = new Uint8Array(n);
  for (const s of sessions) {
    const a = Math.round(wallSeconds(s.start) - t0), b = Math.round(wallSeconds(s.end) - t0);
    for (let i = a; i <= b && i < n; i++) on[i] = 1;
  }
  // Events, consistent with the counts.
  type Ev = { at: number; kind: TherapyEvent["kind"]; code: number; dur: number };
  const evs: Ev[] = [];
  const c = item.ahi == null ? { obstructive: 0, central: 0, hypopnea: 0 } : item.eventCounts;
  for (const event of plan.therapyEvents) evs.push({ at: wallSeconds(event.time) - t0, kind: event.kind, code: event.code, dur: event.value });
  const leakBlocks = plan.leakBlocks;
  for (const [a, b] of leakBlocks) evs.push({ at: a, kind: "largeLeak", code: 0xee, dur: Math.round((b - a) / 60) });
  evs.sort((a, b) => a.at - b.at);

  // Series.
  const pressure = new Float32Array(n);
  const excess = new Float32Array(n);
  const flow = new Float32Array(n);
  const snore = new Float32Array(n);
  const flowDip = new Float32Array(n).fill(1);
  for (const e of evs) {
    if (e.kind === "obstructiveApnea" || e.kind === "centralApnea" || e.kind === "hypopnea") {
      const depth = e.kind === "hypopnea" ? 0.45 : 0.08;
      for (let i = Math.max(0, e.at - e.dur); i < e.at && i < n; i++) flowDip[i] = depth;
    }
  }
  let leakNoise = 0, flowBase = 28;
  const pressureEvents: Ev[] = [];
  let lastLevel = -1;
  for (let i = 0; i < n; i++) {
    const p = mockPressureAt(plan, i);
    pressure[i] = p;
    const level = Math.round(p);
    if (on[i] && level !== lastLevel && i % 60 === 0) {
      if (lastLevel !== -1 && pressureEvents.length < 14) pressureEvents.push({ at: i, kind: "pressureChange", code: 0xe8, dur: level });
      lastLevel = level;
    }
    leakNoise += (r() - 0.5) * 0.25 - leakNoise * 0.01;
    let ex = Math.max(0, leakNoise * 0.7);
    if (!LEAK_NIGHTS.has(plan.index) && i % 900 >= 810) ex = item.excessLeakP95 ?? ex;
    for (const [a, b] of leakBlocks) if (i >= a && i <= b) ex = 38 + r() * 20;
    excess[i] = ex;
    flowBase += (r() - 0.5) * 0.5 - (flowBase - 28) * 0.01;
    flow[i] = Math.max(0, (flowBase + (r() - 0.5) * 3) * flowDip[i]!);
    snore[i] = r() < 0.01 ? Math.round(r() * 3) : 0;
  }

  const toArr = (a: Float32Array, f: (v: number) => number = round1) => {
    const out: (number | null)[] = new Array(n);
    for (let i = 0; i < n; i++) out[i] = on[i] ? f(a[i]!) : null;
    return out;
  };
  const totalArr: (number | null)[] = new Array(n);
  for (let i = 0; i < n; i++) totalArr[i] = on[i] ? round1(excess[i]! + 22 + pressure[i]! * 0.9) : null;

  const allEvents: TherapyEvent[] = [...evs, ...pressureEvents]
    .sort((a, b) => a.at - b.at)
    .map((e) => ({
      time: secondsToWall(t0 + e.at),
      kind: e.kind,
      code: e.code,
      value: e.dur,
      pressure: e.kind === "largeLeak" ? null : round1(pressure[Math.min(n - 1, e.at)]!)
    }));

  const hrs = item.usageMinutes / 60;
  const detail: NightDetail = {
    ...item,
    score: plan.score,
    stages: plan.stages,
    tips: [...nightTips(item, mockNights().filter(n => n.date <= date && n.date >= addDays(date, -6)), plan.index === SETTINGS_CHANGE_INDEX).slice(0, 2), ...pressureTips([mockPressureObservation(date)!])].sort((a, b) => ["talk", "watch", "info", "good"].indexOf(a.severity) - ["talk", "watch", "info", "good"].indexOf(b.severity)).slice(0, 4),
    sessions,
    summary: item.ahi == null
      ? { startPressure: null, maxPressure: null, ai: null, cai: null, oai: null, hi: null, sni: null }
      : {
          startPressure: settings.startPressure,
          maxPressure: round1(Math.max(...Array.from(pressure))),
          ai: round1((c.obstructive + c.central) / hrs),
          cai: round1(c.central / hrs),
          oai: round1(c.obstructive / hrs),
          hi: round1(c.hypopnea / hrs),
          sni: round1(r() * 2)
        },
    settings,
    events: allEvents,
    series: {
      seriesStart: item.start!,
      pressure: toArr(pressure),
      excessLeak: toArr(excess),
      totalLeak: totalArr,
      flowAmplitude: toArr(flow),
      snore: toArr(snore, (v) => v)
    }
  };
  detailCache.set(date, detail);
  return detail;
}

export function mockDaysBetween(a: string, b: string): number {
  return daysInclusive(a, b);
}

function mockPressureAt(plan: NightPlan, second: number): number {
  const { index, settings } = plan;
  if ((index >= 100 && index <= 106 || index === 60) && second % 3600 >= 1200) return settings.maxPressure - 0.1;
  if (LEAK_NIGHTS.has(index) && plan.leakBlocks.some(([a, b]) => second >= a && second <= b)) return settings.maxPressure - 0.2;
  if (index >= 50 && index <= 56) return settings.minPressure + 0.2;
  if (index >= 80 && index <= 82) return settings.minPressure + 0.2;
  const ramp = Math.min(1, second / (settings.rampMinutes * 60));
  const target = plan.item.pressureMean ?? settings.minPressure;
  const amplitude = Math.max(0.1, ((plan.item.pressureP90 ?? target) - target) / 0.95);
  const p = settings.startPressure + (target - settings.startPressure) * ramp + Math.sin(second / 600) * amplitude * ramp;
  return round1(clamp(p, settings.minPressure, settings.maxPressure));
}
export function mockPressureObservation(date: string): PressureObservation | null {
  const plan = plans().find(p => p.item.date === date);
  if (!plan) return null;
  // Count against the same generated pressure values used by the 1 Hz chart.
  const eventsAtMin = plan.therapyEvents.filter(e => ["obstructiveApnea", "hypopnea"].includes(e.kind) && mockPressureAt(plan, wallSeconds(e.time) - wallSeconds(plan.item.start!)) - plan.settings.minPressure <= 0.5).length;
  let atMax = 0, used = 0;
  const start = wallSeconds(plan.item.start!);
  for (const session of plan.sessions) for (let t = wallSeconds(session.start); t < wallSeconds(session.end); t++) {
    const offset = t - start;
    const p = mockPressureAt(plan, offset);
    used++;
    if (plan.settings.maxPressure - p <= 0.5) atMax++;
  }
  return { item: plan.item, settings: plan.settings, shareAtMaxPct: used ? atMax / used * 100 : 0, eventsAtMin };
}
