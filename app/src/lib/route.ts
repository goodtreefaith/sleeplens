import { isDate } from "./dates";

export type Screen = "import" | "overview" | "night" | "trends" | "settings" | "learn";
export interface Route { screen: Screen; date?: string; term?: string }

const SCREENS: Screen[] = ["import", "overview", "night", "trends", "settings", "learn"];

export function parseHash(hash: string): Route {
  const [s, arg] = hash.replace(/^#\/?/, "").split("/");
  const screen = (SCREENS as string[]).includes(s ?? "") ? (s as Screen) : "overview";
  if ((screen === "overview" || screen === "night") && isDate(arg)) return { screen, date: arg };
  if (screen === "learn" && arg) return { screen, term: arg };
  return { screen };
}

export function toHash(r: Route): string {
  if ((r.screen === "overview" || r.screen === "night") && r.date) return `#${r.screen}/${r.date}`;
  if (r.screen === "learn" && r.term) return `#learn/${r.term}`;
  return `#${r.screen}`;
}
