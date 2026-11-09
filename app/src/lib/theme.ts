import { useCallback, useEffect, useLayoutEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark";
const KEY = "sleeplens.theme";

export function readThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(pref: ThemePref) {
  const root = document.documentElement;
  if (pref === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", pref);
  try { localStorage.setItem(KEY, pref); } catch { /* storage unavailable */ }
}

function systemDark() {
  return typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Theme preference plus the theme actually showing (charts need it to pick colours). */
export function useTheme(): [ThemePref, (p: ThemePref) => void, "light" | "dark"] {
  const [pref, setPrefState] = useState<ThemePref>(readThemePref);
  const [sysDark, setSysDark] = useState(systemDark);
  // Apply data-theme before React re-renders, so charts built in effects read the new colours.
  const setPref = useCallback((p: ThemePref) => { applyTheme(p); setPrefState(p); }, []);
  useLayoutEffect(() => {
    applyTheme(pref);
  }, [pref]);
  useEffect(() => {
    if (typeof matchMedia === "undefined") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const on = () => setSysDark(mq.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  const effective = pref === "system" ? (sysDark ? "dark" : "light") : pref;
  return [pref, setPref, effective];
}

/** Reads a CSS custom property's current value. */
export function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}
