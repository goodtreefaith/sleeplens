export type DayPreference = "earlier" | "middle" | "later" | "varies";
export type RhythmLabel = "Morning-leaning" | "Daytime-leaning" | "Evening-leaning" | "Mixed rhythm";
export interface RhythmDraft {
  preference: DayPreference | "";
  bedtime: string;
  wakeTime: string;
  schedule: "regular" | "variable" | "";
  alarm: "free" | "constrained" | "";
  focusStart: string;
  focusEnd: string;
}
export interface RhythmProfile extends RhythmDraft { version: 1 }
export const EMPTY_RHYTHM: RhythmDraft = { preference: "", bedtime: "", wakeTime: "", schedule: "", alarm: "", focusStart: "", focusEnd: "" };
const KEY = "sleeplens.rhythm.v1";
let demoProfile: RhythmProfile | null = null;

export function clockMinutes(value: string): number | null {
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(value)) return null;
  const [hour, minute] = value.split(":").map(Number);
  return hour! * 60 + minute!;
}

export function windowMinutes(start: string, end: string): number | null {
  const from = clockMinutes(start), to = clockMinutes(end);
  return from == null || to == null || from === to ? null : (to - from + 1440) % 1440;
}

export function focusOverlapsSleep(draft: RhythmDraft): boolean {
  function pieces(start: string, end: string): [number, number][] {
    const from = clockMinutes(start), to = clockMinutes(end);
    if (from == null || to == null || from === to) return [];
    return to > from ? [[from, to]] : [[from, 1440], [0, to]];
  }
  return pieces(draft.focusStart, draft.focusEnd).some(focus =>
    pieces(draft.bedtime, draft.wakeTime).some(sleep => focus[0] < sleep[1] && sleep[0] < focus[1]));
}

export function rhythmErrors(draft: RhythmDraft, step?: number): string[] {
  const errors: string[] = [];
  if (step == null || step === 0) {
    if (!["earlier", "middle", "later", "varies"].includes(draft.preference)) errors.push("Choose the part of the day you tend to prefer.");
  }
  if (step == null || step === 1) {
    if (windowMinutes(draft.bedtime, draft.wakeTime) == null) errors.push("Enter different, valid bedtime and wake times. Overnight sleep is fine.");
    if (!["regular", "variable"].includes(draft.schedule)) errors.push("Choose whether your schedule is regular or variable.");
    if (!["free", "constrained"].includes(draft.alarm)) errors.push("Choose whether an alarm or other obligation limits your free-day timing.");
  }
  if (step == null || step === 2) {
    if (windowMinutes(draft.focusStart, draft.focusEnd) == null) errors.push("Enter different, valid start and end times for your chosen focus window.");
  }
  return errors;
}

export function rhythmLabel(preference: DayPreference | ""): RhythmLabel {
  return preference === "earlier" ? "Morning-leaning" : preference === "middle" ? "Daytime-leaning" : preference === "later" ? "Evening-leaning" : "Mixed rhythm";
}

function validatedProfile(value: unknown): RhythmProfile | null {
  if (!value || typeof value !== "object") return null;
  const p = value as Record<string, unknown>;
  if (p.version !== 1 || Object.keys(EMPTY_RHYTHM).some(key => typeof p[key] !== "string")) return null;
  const candidate = p as unknown as RhythmProfile;
  if (rhythmErrors(candidate).length) return null;
  return { version: 1, preference: candidate.preference, bedtime: candidate.bedtime, wakeTime: candidate.wakeTime, schedule: candidate.schedule, alarm: candidate.alarm, focusStart: candidate.focusStart, focusEnd: candidate.focusEnd };
}

export function readRhythmProfile(persistent: boolean): RhythmProfile | null {
  if (!persistent) return demoProfile ? { ...demoProfile } : null;
  try { return validatedProfile(JSON.parse(localStorage.getItem(KEY) ?? "null")); } catch { return null; }
}

export function saveRhythmProfile(draft: RhythmDraft, persistent: boolean): RhythmProfile {
  const profile = validatedProfile({ ...draft, version: 1 });
  if (!profile) throw new Error("Complete the questions with valid times before saving.");
  if (persistent) localStorage.setItem(KEY, JSON.stringify(profile));
  else demoProfile = { ...profile };
  return profile;
}

export function clearRhythmProfile(persistent: boolean): void {
  if (persistent) localStorage.removeItem(KEY);
  demoProfile = null;
}
