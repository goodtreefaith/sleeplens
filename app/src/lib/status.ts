import type { NightListItem, Settings } from "../api/types";

/** good / watch / talk carry the status colour; none means "not recorded" or "not enough". */
export type Status = "good" | "watch" | "talk" | "none";

// General guides from the implementation plan (section 4), never prescriptive.
export function hoursStatus(minutes: number): Status {
  const h = minutes / 60;
  return h >= 7 ? "good" : h >= 4 ? "watch" : "talk";
}

export function ahiStatus(ahi: number | null, usageMinutes?: number): Status {
  if (ahi == null) return "none";
  if (usageMinutes != null && usageMinutes < 10) return "none";
  return ahi < 5 ? "good" : ahi <= 15 ? "watch" : "talk";
}

/** Share of the night with a large leak, in percent (null when not recorded). */
export function largeLeakPct(n: Pick<NightListItem, "largeLeakMinutes" | "usageMinutes">): number | null {
  if (n.largeLeakMinutes == null || n.usageMinutes <= 0) return null;
  return (n.largeLeakMinutes / n.usageMinutes) * 100;
}

export function leakStatus(n: Pick<NightListItem, "largeLeakMinutes" | "usageMinutes">): Status {
  const p = largeLeakPct(n);
  if (p == null) return "none";
  return p < 5 ? "good" : p <= 20 ? "watch" : "talk";
}

export function pressureStatus(n: Pick<NightListItem, "pressureP90" | "pressureMean">, s: Settings | null): Status {
  if (n.pressureP90 == null || !s) return "none";
  if (s.modeLabel === "CPAP") return "good";
  return n.pressureP90 >= s.maxPressure - 0.05 ? "watch" : "good";
}

export const STATUS_WORD = {
  hours: { good: "Good", watch: "Okay", talk: "Short night", none: "No use" },
  ahi: { good: "Normal range", watch: "Watch", talk: "Talk to your clinician", none: "Not recorded" },
  leak: { good: "Good seal", watch: "Some large leak", talk: "A lot of large leak", none: "Not recorded" },
  pressure: { good: "Within your setting", watch: "Reached the top", talk: "Reached the top", none: "Not recorded" }
} as const;

/** The worst of several statuses, ignoring "none". */
export function worst(...ss: Status[]): Status {
  if (ss.includes("talk")) return "talk";
  if (ss.includes("watch")) return "watch";
  if (ss.includes("good")) return "good";
  return "none";
}

/** Settings in effect on a date: the latest change on or before it, else the earliest known. */
export function settingsOn<T extends { date: string }>(history: T[], date: string): T | null {
  let found: T | null = null;
  for (const h of history) {
    if (h.date <= date) found = h;
    else break;
  }
  return found ?? history[0] ?? null;
}

export const SETTING_LABEL: Record<keyof Settings, string> = {
  mode: "Mode",
  modeLabel: "Mode",
  startPressure: "Start pressure",
  minPressure: "Minimum pressure",
  maxPressure: "Maximum pressure",
  fixedPressure: "Fixed pressure",
  rampMinutes: "Ramp",
  humidifierLevel: "Humidifier level",
  comfortLevel: "Comfort level",
  autoOn: "Auto on",
  autoOff: "Auto off",
  leakAlert: "Leak alert",
  pressureUnit: "Pressure unit"
};
