import type { SleepScore } from "../api/types";

/** Bar widths stay valid even for rounded or incomplete backend component data. */
export function scoreBreakdown(score: SleepScore | null) {
  return (score?.components ?? []).map(component => ({
    ...component,
    percent: component.maxPoints > 0 && Number.isFinite(component.points)
      ? Math.max(0, Math.min(100, component.points / component.maxPoints * 100)) : 0
  }));
}
