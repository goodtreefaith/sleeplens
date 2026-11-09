import { wallSeconds } from "../lib/dates";
import type { NightListItem, Session, SleepScore, SleepStageEstimate, SleepStage, StageMinutes, Tip } from "./types";

const clamp = (n: number, max: number) => Math.max(0, Math.min(max, n));
const round = (n: number) => Math.round(n * 10) / 10;
function linear(x: number, points: [number, number][]): number {
  for (let i = 1; i < points.length; i++) {
    const [a, y] = points[i - 1]!;
    const [b, z] = points[i]!;
    if (x <= b) return y + clamp((x - a) / (b - a), 1) * (z - y);
  }
  return points[points.length - 1]![1];
}

/** Synthetic 30-second epochs, preserving mask-off gaps in the shared wall-clock axis. */
export function estimateMockStages(item: NightListItem, sessions: Session[], index: number): SleepStageEstimate {
  const start = wallSeconds(item.start!);
  const epochs = Math.ceil((wallSeconds(item.end!) - start) / 30);
  const minutes: StageMinutes = { wake: 0, light: 0, deep: 0, rem: 0 };
  const cycles = item.usageMinutes > 390 ? 5 : 4;
  const cycleLength = item.usageMinutes / cycles;
  let maskMinutes = 0;
  const stages: SleepStage[] = [];
  for (let e = 0; e < epochs; e++) {
    const t = start + e * 30;
    const on = sessions.some(s => t >= wallSeconds(s.start) && t < wallSeconds(s.end));
    let stage: SleepStage = "noData";
    if (on) {
      const cycle = Math.min(cycles - 1, Math.floor(maskMinutes / cycleLength));
      const phase = (maskMinutes % cycleLength) / cycleLength;
      const deepEnd = Math.max(0.12, 0.34 - cycle * 0.035);
      const remStart = 0.88 - cycle * 0.03;
      stage = maskMinutes < 12 || (e + index * 7) % 193 < 3 ? "wake"
        : phase > 0.10 && phase < deepEnd ? "deep"
        : phase > remStart ? "rem" : "light";
      minutes[stage] += 0.5;
      maskMinutes += 0.5;
    }
    stages.push(stage);
  }
  return { start: item.start!, epochSeconds: 30, stages, minutes, signalQuality: round(0.82 + (index % 7) * 0.02), estimatorVersion: 1 };
}

export function computeMockScore(item: NightListItem, stages: SleepStageEstimate | null): SleepScore | null {
  if (item.usageMinutes < 30) return null;
  const components: SleepScore["components"] = [];
  let unroundedPoints = 0;
  const add = (key: SleepScore["components"][number]["key"], label: string, points: number, maxPoints: number, detail: string) => {
    unroundedPoints += points;
    components.push({ key, label, points: round(points), maxPoints, detail });
  };
  add("duration", "Hours used", linear(item.usageMinutes, [[0, 0], [240, 12], [360, 22], [420, 27], [480, 30]]), 30, `${Math.floor(item.usageMinutes / 60)} h ${item.usageMinutes % 60} m of use, compared with the 8 h guide.`);
  if (item.ahi != null) add("breathing", "Breathing events", linear(item.ahi, [[0, 30], [1, 28], [2, 26], [5, 20], [15, 8], [30, 0]]), 30, `${item.ahi.toFixed(1)} breathing events per hour were recorded.`);
  if (item.largeLeakMinutes != null) {
    const leakShare = (item.largeLeakMinutes ?? 0) / item.usageMinutes * 100;
    const points = linear(leakShare, [[0, 15], [5, 10], [20, 0]]) - clamp(((item.excessLeakP95 ?? 0) - 20) / 16 * 5, 5);
    add("maskSeal", "Mask seal", Math.max(0, points), 15, `${item.largeLeakMinutes == null ? "Large-leak time was not recorded" : `${item.largeLeakMinutes} minutes of large leak`}; ${item.excessLeakP95 == null ? "extra-leak data was not recorded" : `extra leak reached ${item.excessLeakP95} L/min at the 95th percentile`}.`);
  }
  const wakePenalty = stages ? clamp((stages.minutes.wake / item.usageMinutes - 0.05) / 0.15 * 6, 6) : 0;
  add("continuity", "Unbroken sleep", clamp(15 - item.interruptions * 3 - wakePenalty, 15), 15, `${item.interruptions} mask-off ${item.interruptions === 1 ? "break interrupted" : "breaks interrupted"} this night.`);
  if (stages) {
    const sleep = stages.minutes.light + stages.minutes.deep + stages.minutes.rem;
    add("sleepStages", "Sleep stages (estimated)", 5 * Math.min(1, sleep > 0 ? stages.minutes.deep / sleep / 0.15 : 0) + 5 * Math.min(1, sleep > 0 ? stages.minutes.rem / sleep / 0.20 : 0), 10, `${Math.round(stages.minutes.deep + stages.minutes.rem)} minutes were estimated as deep or REM sleep.`);
  }
  const available = components.reduce((sum, c) => sum + c.maxPoints, 0);
  const total = Math.round(unroundedPoints / available * 100);
  return { total, label: total >= 85 ? "Great" : total >= 70 ? "Good" : total >= 50 ? "Fair" : "Poor", estimated: true, components };
}

export function nightTips(item: NightListItem, recent: NightListItem[] = [], prescriptionChanged = false): Tip[] {
  const tips: Tip[] = [];
  const add = (id: string, severity: Tip["severity"], title: string, body: string, learnSlug: string) => tips.push({ id, severity, title, body, learnSlug, evidence: id === "ahi-watch" ? `AHI ${item.ahi?.toFixed(1)} events/hour` : id === "mask-fit" ? `${item.largeLeakMinutes ?? 0} minutes of large leak; extra leak P95 ${item.excessLeakP95 ?? "not recorded"} L/min` : id === "short-night" ? `${Math.round(item.usageMinutes)} minutes of use` : id === "interruptions" ? `${item.interruptions} mask-off breaks` : id === "central-watch" ? `${item.eventCounts.central} central events recorded` : "Prescription changed on this night", evidenceDates: [item.date] });
  const totalEvents = Object.values(item.eventCounts).reduce((a, b) => a + b, 0);
  if (item.eventCounts.central / (item.usageMinutes / 60) > 5 || (item.ahi != null && item.ahi > 5 && item.eventCounts.central > totalEvents / 2)) add("central-watch", "talk", "Discuss central breathing events", "Contact your clinician about the central breathing events recorded during therapy.", "centralApnea");
  if (item.ahi != null && item.ahi >= 5) {
    const repeated = recent.filter(n => n.ahi != null && n.ahi >= 5).length >= 3;
    const talk = item.ahi > 15 || repeated;
    add("ahi-watch", talk ? "talk" : "watch", talk ? "Discuss your breathing events" : "Keep an eye on breathing events", talk ? "Contact your clinician to review the breathing events recorded during therapy." : "Watch the pattern over the next few nights. Contact your clinician if it continues.", "ahi");
  }
  if (item.usageMinutes < 240) add("short-night", "watch", "Keep your mask on through the night", "Aim to use therapy whenever you sleep. Auto ON can help when you put your mask back on.", "validNight");
  if ((item.largeLeakMinutes ?? 0) / item.usageMinutes > 0.05 || (item.excessLeakP95 ?? 0) > 24) add("mask-fit", "watch", "Check your mask fit", "Check the fit of your mask and look for cushion wear. A comfortable seal helps therapy work as intended.", "excessLeak");
  if (item.interruptions >= 3) add("interruptions", "info", "Several mask-off breaks", "There were several breaks in therapy. Notice whether discomfort or your routine made it harder to keep the mask on.", "interruptions");
  if (prescriptionChanged) add("prescription-changed", "info", "Compare before and after", "Your prescription changed. Compare several nights before and after the change with your clinician.", "pressure");
  return tips.length ? tips.slice(0, 4) : [{ id: "all-good", severity: "good", title: "Keep your routine going", body: "Your recorded therapy measures look steady. Keep using your mask whenever you sleep.", learnSlug: "sleepScore", evidence: `AHI ${item.ahi == null ? "not recorded" : item.ahi.toFixed(1) + " events/hour"}; ${Math.round(item.usageMinutes)} minutes of use`, evidenceDates: [item.date] }];
}
