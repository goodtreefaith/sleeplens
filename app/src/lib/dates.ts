// Dates are "YYYY-MM-DD" and times local wall-clock "YYYY-MM-DDTHH:MM:SS" with no zone.
// We do all arithmetic in UTC so the wall-clock value is never shifted by the computer's zone.

export const DAY_MS = 86_400_000;

export function dateMs(d: string): number {
  return Date.parse(d + "T00:00:00Z");
}

export function msDate(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

export function addDays(d: string, k: number): string {
  return msDate(dateMs(d) + k * DAY_MS);
}

/** Days from a to b inclusive. */
export function daysInclusive(a: string, b: string): number {
  return Math.round((dateMs(b) - dateMs(a)) / DAY_MS) + 1;
}

/** Wall-clock time as seconds since the Unix epoch, read as if UTC (so charts show it unshifted). */
export function wallSeconds(t: string): number {
  const iso = t.length === 16 ? t + ":00" : t;
  return Date.parse(iso + "Z") / 1000;
}

export function secondsToWall(s: number): string {
  return new Date(s * 1000).toISOString().slice(0, 19);
}

/** True for a real calendar date in YYYY-MM-DD form (rejects 2026-02-30). */
export function isDate(s: string | undefined | null): s is string {
  if (!s || !/^\d{4}-\d{2}-\d{2}$/.test(s)) return false;
  const ms = Date.parse(s + "T00:00:00Z");
  return Number.isFinite(ms) && new Date(ms).toISOString().slice(0, 10) === s;
}
