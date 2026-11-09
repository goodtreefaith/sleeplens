import type { SleepStage, StageMinutes } from "../api/types";

export type StageMode = "simple" | "detailed";
export type DisplayStage = SleepStage | "nonRem";
export const STAGE_MODE_STORAGE_KEY = "sleeplens-stage-mode";
export const TYPICAL_STAGE_RANGES = { wake: [0, 10], nonRem: [65, 80], rem: [20, 25], light: [45, 60], deep: [13, 23] } as const;

export function displayStage(stage: SleepStage, mode: StageMode): DisplayStage {
  return mode === "simple" && (stage === "light" || stage === "deep") ? "nonRem" : stage;
}
export function displayStageMinutes(minutes: StageMinutes, mode: StageMode) {
  return mode === "simple" ? { wake: minutes.wake, nonRem: minutes.light + minutes.deep, rem: minutes.rem, uncertain: minutes.uncertain ?? 0 }
    : { wake: minutes.wake, light: minutes.light, deep: minutes.deep, rem: minutes.rem, uncertain: minutes.uncertain ?? 0 };
}
/** Uncertain means recorded but confounded; noData means no recorded signal. */
export function stagePattern(stage: DisplayStage): "hatched" | "empty-dashed" | "solid" {
  return stage === "uncertain" ? "hatched" : stage === "noData" ? "empty-dashed" : "solid";
}
export function readStageMode(): StageMode {
  try { return typeof window !== "undefined" && window.localStorage.getItem(STAGE_MODE_STORAGE_KEY) === "detailed" ? "detailed" : "simple"; }
  catch { return "simple"; }
}
export function stageRuns<T extends string>(stages: readonly T[]) {
  const runs: { stage: T; first: number; end: number }[] = [];
  stages.forEach((stage, i) => {
    const previous = runs[runs.length - 1];
    if (previous?.stage === stage) previous.end = i + 1;
    else runs.push({ stage, first: i, end: i + 1 });
  });
  return runs;
}
export function countSleepCycles(stages: readonly SleepStage[]): number {
  const periods = stageRuns(stages).filter(run => run.stage === "rem").length;
  return Math.max(periods, stages.some(stage => stage === "light" || stage === "deep") ? 1 : 0);
}
export function stageShare(minutes: StageMinutes, stage: keyof StageMinutes | "nonRem"): number {
  const total = Object.values(minutes).reduce((sum, value) => sum + Math.max(0, value), 0);
  const value = stage === "nonRem" ? minutes.light + minutes.deep : minutes[stage] ?? 0;
  return total ? Math.max(0, value) / total * 100 : 0;
}
export function stageRangeStatus(stage: keyof typeof TYPICAL_STAGE_RANGES, share: number): "Below typical range" | "Within typical range" | "Above typical range" {
  const [low, high] = TYPICAL_STAGE_RANGES[stage];
  return share < low ? "Below typical range" : share > high ? "Above typical range" : "Within typical range";
}

export function roundedStageDuration(minutes: number): string {
  const roundedMinutes = Math.round(Math.max(0, minutes) / 5) * 5;
  if (roundedMinutes < 60) return `about ${roundedMinutes} min`;
  const quarters = Math.round(roundedMinutes / 15);
  const hours = Math.floor(quarters / 4);
  const fraction = ["", "¼", "½", "¾"][quarters % 4];
  return `about ${hours}${fraction ? " " + fraction : ""} h`;
}
