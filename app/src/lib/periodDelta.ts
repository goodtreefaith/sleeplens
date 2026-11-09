import type { Period } from "../api/types";
const PREVIOUS: Record<Period, string> = { week: "last week", month: "last month", "6months": "the previous 6 months", year: "last year" };

/** Positive changes help usage and score; negative changes help AHI. */
export function periodDelta(current: number | null, previous: number | null, period: Period, suffix = "", lowerIsBetter = false, decimals = 1) {
  if (current == null) return { tone: "neutral", icon: "○", text: "No recorded value to compare" };
  if (previous == null) return { tone: "neutral", icon: "○", text: "No previous period to compare" };
  const factor = 10 ** decimals;
  const delta = Math.round((current - previous) * factor) / factor;
  if (delta === 0) return { tone: "neutral", icon: "→", text: `About the same as ${PREVIOUS[period]}` };
  const better = lowerIsBetter ? delta < 0 : delta > 0;
  return { tone: better ? "better" : "worse", icon: delta > 0 ? "↑" : "↓", text: `${Math.abs(delta)}${suffix} ${delta > 0 ? "higher" : "lower"}, ${better ? "better" : "worse"} than ${PREVIOUS[period]}` };
}
