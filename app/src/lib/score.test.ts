import { describe, expect, it } from "vitest";
import { scoreBreakdown } from "./score";
import type { SleepScore } from "../api/types";

describe("score breakdown", () => {
  it("preserves labels and details while calculating bounded bar widths", () => {
    const component = { key: "duration" as const, label: "Hours used", points: 15, maxPoints: 30, detail: "Four hours recorded." };
    const score: SleepScore = { total: 50, label: "Fair", estimated: true, components: [component] };
    expect(scoreBreakdown(score)).toEqual([{ ...component, percent: 50 }]);
    score.components = [{ ...component, points: 100 }, { ...component, points: -1 }, { ...component, maxPoints: 0 }];
    expect(scoreBreakdown(score).map(c => c.percent)).toEqual([100, 0, 0]);
    expect(scoreBreakdown(null)).toEqual([]);
  });
});
