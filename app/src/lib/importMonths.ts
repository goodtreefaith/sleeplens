import type { ImportProgress, ImportResult } from "../api/types";

export type MonthStatus = "Waiting" | "Reading…" | "Done" | "Skipped — already imported";
export type ImportMonths = Record<string, MonthStatus>;

/** Keep observed month transitions across navigation. A new current month completes the previous one. */
export function advanceImportMonths(months: ImportMonths, progress: ImportProgress): ImportMonths {
  if (progress.phase === "done") return Object.fromEntries(Object.keys(months).map(month => [month, "Done"]));
  const month = progress.currentDate?.match(/^(\d{4}-\d{2})-\d{2}/)?.[1];
  if (progress.phase !== "nights" || !month) return months;
  const next = { ...months };
  for (const key of Object.keys(next)) if (next[key] === "Reading…" && key !== month) next[key] = "Done";
  next[month] = progress.done >= progress.total && progress.total > 0 ? "Done" : "Reading…";
  return next;
}

export function finishImportMonths(months: ImportMonths, result: ImportResult): ImportMonths {
  if (result.status !== "ok") return months;
  // An aggregate skip count proves all months were already imported only when every night was skipped.
  const allSkipped = result.nightsSkipped > 0 && !result.nightsAdded && !result.nightsUpdated && !result.nightsFailed.length;
  return Object.fromEntries(Object.keys(months).map(month => [month, allSkipped ? "Skipped — already imported" : "Done"]));
}
