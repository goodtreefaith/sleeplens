import { dateMs } from "./dates";
import type { PressureUnit } from "../api/types";

const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WD = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const parts = (d: string) => {
  const dt = new Date(dateMs(d));
  return { day: dt.getUTCDate(), mon: MON[dt.getUTCMonth()]!, year: dt.getUTCFullYear(), wd: WD[dt.getUTCDay()]! };
};

const MONTH = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAY = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** "Sunday 8 November" (built by hand so it never depends on the system locale) */
export const longDate = (d: string) => { const t = new Date(dateMs(d)); return `${WEEKDAY[t.getUTCDay()]} ${t.getUTCDate()} ${MONTH[t.getUTCMonth()]}`; };
/** "Sunday 8 November 2026" */
export const longDateY = (d: string) => `${longDate(d)} ${d.slice(0, 4)}`;
/** "8 Nov" (three-letter months everywhere: "8 Sep", never "8 Sept") */
export const shortDate = (d: string) => { const p = parts(d); return `${p.day} ${p.mon}`; };
/** "8 Nov 2026" */
export const shortDateY = (d: string) => { const p = parts(d); return `${p.day} ${p.mon} ${p.year}`; };
/** "Sun 8 Nov 2026" */
export const midDate = (d: string) => { const p = parts(d); return `${p.wd} ${p.day} ${p.mon} ${p.year}`; };
/** "Sun 8 Nov" */
export const dayDate = (d: string) => { const p = parts(d); return `${p.wd} ${p.day} ${p.mon}`; };
/** "November 2026" */
export const monthYear = (d: string) => `${MONTH[new Date(dateMs(d)).getUTCMonth()]} ${d.slice(0, 4)}`;

/** Minutes as "7 h 12 m", "45 m", or "under 1 m". */
export function hm(min: number | null | undefined): string {
  if (min == null || !Number.isFinite(min)) return "–";
  if (min > 0 && min < 1) return "under 1 m";
  const t = Math.round(min);
  const h = Math.floor(t / 60);
  const m = t % 60;
  return h ? `${h} h ${m} m` : `${m} m`;
}

/** One decimal place, or "–" when missing. */
export function r1(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return "–";
  return (Math.round(v * 10) / 10).toFixed(1);
}

/** Clock "HH:MM" from a local wall-clock time string. */
export function clock(t: string | null | undefined): string {
  return t ? t.slice(11, 16) : "–";
}

/** Display label for a pressure unit. */
export function unitLabel(u: PressureUnit | null | undefined): string {
  if (u === "cmH2O") return "cmH₂O";
  if (u === "hPa") return "hPa";
  return "";
}

/** Spoken unit, for screen readers. */
export function unitSpoken(u: PressureUnit | null | undefined): string {
  if (u === "cmH2O") return "centimetres of water";
  if (u === "hPa") return "hectopascals";
  return "";
}

export function bytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "–";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function plural(n: number, one: string, many = one + "s"): string {
  return `${n} ${n === 1 ? one : many}`;
}

export function pct(v: number): string {
  return `${Math.round(v)}%`;
}
