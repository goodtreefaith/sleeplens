import type { SleepScore } from "../api/types";

export type ScoreComponentKey = SleepScore["components"][number]["key"];
export const SCORE_COMPONENTS = [
  { key: "duration", label: "Hours used", maxPoints: 30, color: "var(--score-duration)" },
  { key: "breathing", label: "Breathing events", maxPoints: 30, color: "var(--score-breathing)" },
  { key: "maskSeal", label: "Mask seal", maxPoints: 15, color: "var(--score-mask-seal)" },
  { key: "continuity", label: "Unbroken sleep", maxPoints: 15, color: "var(--score-continuity)" },
  { key: "sleepStages", label: "REM sleep (estimated, low confidence)", maxPoints: 10, color: "var(--score-stages)" }
] as const;

export function contributorStatus(points: number, maximum: number): string {
  const share = maximum > 0 && Number.isFinite(points) ? points / maximum : 0;
  return share >= .9 ? "Optimal" : share >= .7 ? "Good" : share >= .45 ? "Fair" : "Pay attention";
}

/** Fixed weighted tracks preserve missing contributors and each component's original weight. */
export function scoreSegments(score: SleepScore | null, radius = 43, gap = 4) {
  const circumference = 2 * Math.PI * radius;
  const available = Math.max(0, circumference - gap * SCORE_COMPONENTS.length);
  let offset = 0;
  return SCORE_COMPONENTS.map(definition => {
    const component = score?.components.find(c => c.key === definition.key);
    const length = available * definition.maxPoints / 100;
    const share = component && component.maxPoints > 0 && Number.isFinite(component.points)
      ? Math.max(0, Math.min(1, component.points / component.maxPoints)) : 0;
    const segment = { ...definition, length, fillLength: length * share, offset, circumference, component, share };
    offset += length + gap;
    return segment;
  });
}
