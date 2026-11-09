import type { Period } from "../api/types";
import { isDate } from "./dates";

export type Screen = "import" | "overview" | "night" | "trends" | "settings" | "learn" | "rhythm";
export interface Route { screen: Screen; date?: string; term?: string; period?: Period | "all" }

const SCREENS: Screen[] = ["import", "overview", "night", "trends", "settings", "learn", "rhythm"];

export function parseHash(hash: string): Route {
  const [path, query] = hash.replace(/^#\/?/, "").split("?");
  const [s, arg] = (path ?? "").split("/");
  const screen = (SCREENS as string[]).includes(s ?? "") ? (s as Screen) : "overview";
  if (screen === "trends") {
    const params = new URLSearchParams(query);
    const period = params.get("period");
    return { screen, ...(isDate(arg) ? { date: arg } : {}), ...(["week", "month", "6months", "year", "all"].includes(period ?? "") ? { period: period as Period | "all" } : {}) };
  }
  if ((screen === "overview" || screen === "night") && isDate(arg)) return { screen, date: arg };
  if (screen === "learn" && arg) return { screen, term: arg };
  return { screen };
}

export function toHash(r: Route): string {
  if (r.screen === "trends") return `#trends${r.date ? `/${r.date}` : ""}${r.period ? `?period=${r.period}` : ""}`;
  if ((r.screen === "overview" || r.screen === "night") && r.date) return `#${r.screen}/${r.date}`;
  if (r.screen === "learn" && r.term) return `#learn/${r.term}`;
  return `#${r.screen}`;
}
