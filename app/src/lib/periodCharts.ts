import type { Period, StageMinutes } from "../api/types";
import { addDays, daysInclusive } from "./dates";
import { shortDateY } from "./format";

export function periodTitle(period: Period, from: string, to: string, latest: string | undefined) {
  if (to !== latest) return `${shortDateY(from)} to ${shortDateY(to)}`;
  return { week: "This week", month: "This month", "6months": "Last 6 months", year: "This year" }[period];
}

export type RestorativePoint = { date: string; end: string; deep: number | null; rem: number | null; nights: number; pickDate: string | null };
/** Long periods use means over recorded stage estimates, never treat missing estimates as zero. */
export function restorativePoints(from: string, to: string, period: Period, minutes: ReadonlyMap<string, StageMinutes | null>): RestorativePoint[] {
  const step = period === "week" || period === "month" ? 1 : 7;
  const points: RestorativePoint[] = [];
  for (let offset = 0; offset < daysInclusive(from, to); offset += step) {
    const date = addDays(from, offset), end = addDays(date, step - 1) > to ? to : addDays(date, step - 1);
    let deep = 0, rem = 0, nights = 0, pickDate: string | null = null;
    for (let d = date; d <= end; d = addDays(d, 1)) {
      const value = minutes.get(d);
      if (value) { pickDate ??= d; deep += value.deep; rem += value.rem; nights++; }
    }
    points.push({ date, end, deep: nights ? deep / nights : null, rem: nights ? rem / nights : null, nights, pickDate });
  }
  return points;
}

/** Estimate rendered label bounds and keep the final date when labels compete for space. */
export function periodTickIndices(labels: readonly string[], week: boolean, plotWidth: number): number[] {
  const count = labels.length;
  if (!count) return [];
  const step = plotWidth / count;
  const bounds = (i: number) => {
    const width = labels[i]!.length * 7;
    const x = (i + .5) * step;
    const left = !week && i === 0 ? x : !week && i === count - 1 ? x - width : x - width / 2;
    return { left, right: left + width };
  };
  const ticks: number[] = [];
  for (let i = 0; i < count; i++) {
    if (week || i === 0 || i === count - 1 || (count <= 31 && i % 7 === 0)) {
      const current = bounds(i);
      if (i === count - 1) {
        while (ticks.length && current.left - bounds(ticks[ticks.length - 1]!).right < 12) ticks.pop();
      }
      if (!ticks.length || current.left - bounds(ticks[ticks.length - 1]!).right >= 12) ticks.push(i);
    }
  }
  return ticks;
}
