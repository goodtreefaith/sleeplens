import { plural } from "../lib/format";
import type { NightListItem, PressureSummary, Settings, Tip } from "./types";

export interface PressureObservation {
  item: NightListItem;
  settings: Settings;
  shareAtMaxPct: number;
  eventsAtMin: number;
  /** Machine summary CAI, using the same precision as the displayed summary. */
  centralIndex?: number;
  shareAtMinPct?: number;
  /** Only populate these means after at least 60 valid recorded seconds in each pressure group. */
  highPressureLeakMean?: number;
  lowPressureLeakMean?: number;
}
const average = (values: number[]) => values.length ? values.reduce((sum, value) => sum + value, 0) / values.length : null;
export function pressureSummary(observations: PressureObservation[]): PressureSummary | null {
  if (!observations.length) return null;
  const latest = observations[observations.length - 1]!.settings;
  const totalEvents = observations.reduce((sum, n) => sum + n.item.eventCounts.obstructive + n.item.eventCounts.hypopnea, 0);
  return {
    settingMin: latest.minPressure, settingMax: latest.maxPressure,
    avgP90: average(observations.flatMap(n => n.item.pressureP90 == null ? [] : [n.item.pressureP90])),
    nightsNearMax: observations.filter(n => n.item.pressureP90 != null && n.item.pressureP90 >= n.settings.maxPressure - 0.25 || n.shareAtMaxPct >= 10).length,
    nightsNearMin: observations.filter(n => n.item.pressureP90 != null && n.item.pressureP90 <= n.settings.minPressure + 0.25 || (n.shareAtMinPct ?? 0) >= 50).length,
    avgShareAtMaxPct: average(observations.map(n => n.shareAtMaxPct)),
    eventsAtMinPct: totalEvents ? observations.reduce((sum, n) => sum + n.eventsAtMin, 0) / totalEvents * 100 : null
  };
}

/** These are observations for a review, never instructions to change therapy. */
export function pressureTips(observations: PressureObservation[], period = false): Tip[] {
  observations = [...new Map(observations.filter(n => n.item.usageMinutes > 0).map(n => [n.item.date, n])).values()].sort((a, b) => a.item.date.localeCompare(b.item.date));
  if (!observations.length || (period && observations.length < 7)) return [];
  const tips: Tip[] = [];
  const add = (id: string, severity: Tip["severity"], title: string, body: string, matches: PressureObservation[], evidence: string, learnSlug = "pressureInsights") => tips.push({
    category: "pressure", id, severity, title, body, learnSlug, evidence,
    evidenceDates: matches.slice(-5).reverse().map(n => n.item.date)
  });
  const eligible = observations.filter(n => ["Auto CPAP", "AUTOS"].includes(n.settings.modeLabel) && n.settings.minPressure > 0 && n.settings.maxPressure > n.settings.minPressure);
  const roundedIndex = (count: number, n: PressureObservation) => Math.round(count / (n.item.usageMinutes / 60) * 10) / 10;
  const rate = (n: PressureObservation) => (n.item.eventCounts.obstructive + n.item.eventCounts.hypopnea) / (n.item.usageMinutes / 60);
  const obstructionIndex = (n: PressureObservation) => n.item.ahi == null ? n.item.eventCounts.obstructive / (n.item.usageMinutes / 60) : roundedIndex(n.item.eventCounts.obstructive, n);
  const burden = (n: PressureObservation) => n.item.ahi == null ? rate(n) : roundedIndex(n.item.eventCounts.obstructive, n) + roundedIndex(n.item.eventCounts.hypopnea, n);
  const ceiling = eligible.filter(n => n.item.pressureP90 != null && n.item.pressureP90 >= n.settings.maxPressure - 0.25 || n.shareAtMaxPct >= 10);
  const ceilingEvents = (average(ceiling.flatMap(n => n.item.ahi == null ? [] : [n.item.ahi])) ?? 0) >= 5 || (average(ceiling.map(obstructionIndex)) ?? 0) >= 2;
  const floorEvents = eligible.filter(n => {
    const total = n.item.eventCounts.obstructive + n.item.eventCounts.hypopnea;
    return total >= 5 && n.eventsAtMin / total >= 0.3;
  });
  const central = observations.filter(n => (n.centralIndex ?? (n.item.ahi == null ? 0 : roundedIndex(n.item.eventCounts.central, n))) > 5 || ((n.item.ahi ?? 0) > 5 && n.item.eventCounts.central > Object.values(n.item.eventCounts).reduce((a, b) => a + b, 0) / 2));
  const contrast = (high: number | null | undefined, low: number | null | undefined) => high != null && low != null && high > 24 && high - low >= 10;
  const stretches = observations.filter(n => contrast(n.highPressureLeakMean, n.lowPressureLeakMean));
  const high = eligible.filter(n => n.item.pressureP90 != null && n.item.pressureP90 >= n.settings.minPressure + (n.settings.maxPressure - n.settings.minPressure) * 2 / 3);
  const low = eligible.filter(n => n.item.pressureP90 != null && n.item.pressureP90 <= n.settings.minPressure + (n.settings.maxPressure - n.settings.minPressure) / 3);
  const leakMean = (ns: PressureObservation[]) => average(ns.flatMap(n => n.item.excessLeakP95 == null ? [] : [n.item.excessLeakP95]));
  const leakShare = (ns: PressureObservation[]) => average(ns.flatMap(n => n.item.largeLeakMinutes == null ? [] : [100 * n.item.largeLeakMinutes / n.item.usageMinutes]));
  const highShare = leakShare(high), lowShare = leakShare(low);
  const periodLeak = period && (contrast(leakMean(high), leakMean(low)) || (highShare != null && lowShare != null && highShare > 5 && highShare - lowShare >= 5));
  const leaks = periodLeak ? [...high, ...low] : stretches;
  const nearMin = eligible.filter(n => (n.item.pressureP90 != null && n.item.pressureP90 <= n.settings.minPressure + 0.25) || (n.shareAtMinPct ?? 0) >= 50);
  const lowUse = nearMin.length / observations.length >= 0.7 && observations.every(n => n.item.ahi != null && n.item.ahi < 5 && burden(n) < 2 && rate(n) < 2);
  const floorBurden = average(floorEvents.map(burden)) ?? 0;
  const floorWatch = floorBurden >= 3 || (average(floorEvents.flatMap(n => n.item.ahi == null ? [] : [n.item.ahi])) ?? 0) >= 5;
  if (central.length) add("central-with-pressure", "talk", "Discuss central events and pressure", "Central breathing events were recorded during therapy. Ask your clinician whether pressure or another cause could be involved.", central, `Central events met the review guide on ${plural(central.length, "night")}.`, "treatmentEmergentCentralApnea");
  if (ceiling.length / observations.length >= 0.3) {
    if (ceilingEvents) add("pressure-ceiling-events", "talk", "Events while near the maximum", "The machine used the top of its prescribed range while breathing events were still recorded. Ask your clinician whether the maximum still suits you.", ceiling, `P90 near the maximum with residual events on ${plural(ceiling.length, "night")}.`);
    else if (ceiling.every(n => n.item.ahi != null && n.item.ahi < 5) && (period || ceiling.every(n => n.shareAtMaxPct >= 20))) add("pressure-ceiling", "info", "Pressure near the top of the range", "The machine used the top of its prescribed range with few breathing events. Mention this pattern at your next review.", ceiling, `P90 near the maximum on ${plural(ceiling.length, "night")}.`);
  }
  if (period && floorEvents.length >= 3 && (floorWatch || (floorBurden >= 1 && !lowUse))) add("pressure-floor-events", floorWatch ? "watch" : "info", "Events at the minimum pressure", floorWatch ? "Obstructive apneas or hypopneas often happened near the recorded minimum, including any ramp time at that pressure. Ask your clinician whether the minimum and start settings still suit you." : "Some breathing events happened at the lowest pressure, often early in the night. Mention this at your next review.", floorEvents, `At least five recorded obstructive events and hypopneas, with at least 30% near the minimum, on ${plural(floorEvents.length, "night")}; average obstructive apnea plus hypopnea index ${floorBurden.toFixed(1)}.`);
  if (leaks.length) add("leak-at-high-pressure", "watch", "Leak during high pressure", "Leak was greater when pressure was high. Check mask fit with your equipment provider or clinician and discuss this pattern.", leaks, periodLeak ? "Mean excess-leak p95 was above 24 L/min and at least 10 L/min above comparison nights, or large-leak share was above 5% and at least 5 percentage points above comparison nights." : "Mean excess leak was above 24 L/min in high-pressure stretches and at least 10 L/min above low-pressure stretches, with at least 60 recorded seconds in each group.");
  if (lowUse && !(period && floorEvents.length >= 3 && floorWatch)) add("pressure-low-use", "info", "Pressure stayed near the minimum", "Pressure rarely moved beyond the minimum and few breathing events were recorded. Mention this pattern and the prescribed range at your next review.", nearMin, `P90 near the minimum with AHI under 5 on ${plural(nearMin.length, "night")}.`);
  if (!tips.length) {
    const ok = eligible.filter(n => n.item.ahi != null && n.item.ahi < 5 && n.item.pressureP90 != null && n.item.pressureP90 >= n.settings.minPressure && n.item.pressureP90 < n.settings.maxPressure - 0.25 && n.item.excessLeakP95 != null && n.item.excessLeakP95 <= 24 && n.item.largeLeakMinutes != null && n.item.largeLeakMinutes / n.item.usageMinutes <= 0.05 && n.shareAtMaxPct < 10);
    if (ok.length === observations.length) add("pressure-ok", "good", "The pressure range looks steady", "The pressure range looks like it is doing its job. Keep this record for your routine clinician review.", ok, `AHI under 5, P90 inside the range and no substantial leak on ${plural(ok.length, "night")}.`);
  }
  return tips.sort((a, b) => ["talk", "watch", "info", "good"].indexOf(a.severity) - ["talk", "watch", "info", "good"].indexOf(b.severity)).slice(0, 2);
}
