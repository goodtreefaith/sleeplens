import type { SleepStage, StageMinutes } from "../api/types";

export const TYPICAL_STAGE_RANGES = { wake: [0, 10], rem: [20, 25], light: [45, 60], deep: [13, 23] } as const;

export function stageRuns(stages: readonly SleepStage[]) {
  const runs: { stage: SleepStage; first: number; end: number }[] = [];
  stages.forEach((stage, i) => {
    const previous = runs[runs.length - 1];
    if (previous?.stage === stage) previous.end = i + 1;
    else runs.push({ stage, first: i, end: i + 1 });
  });
  return runs;
}

/** REM runs approximate completed cycles. A night with sleep has at least one. */
export function countSleepCycles(stages: readonly SleepStage[]): number {
  const periods = stageRuns(stages).filter(run => run.stage === "rem").length;
  return Math.max(periods, stages.some(stage => stage === "light" || stage === "deep") ? 1 : 0);
}

export function stageShare(minutes: StageMinutes, stage: keyof StageMinutes): number {
  const total = Object.values(minutes).reduce((sum, value) => sum + Math.max(0, value), 0);
  return total ? Math.max(0, minutes[stage]) / total * 100 : 0;
}

export function stageRangeStatus(stage: keyof StageMinutes, share: number): "Below typical range" | "Within typical range" | "Above typical range" {
  const [low, high] = TYPICAL_STAGE_RANGES[stage];
  return share < low ? "Below typical range" : share > high ? "Above typical range" : "Within typical range";
}
