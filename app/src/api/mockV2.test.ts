import { describe, expect, it } from "vitest";
import { computeMockScore, nightTips } from "./mockV2";
import { mockNights } from "./mockData";
import type { NightListItem, SleepStageEstimate } from "./types";

const night = (changes: Partial<NightListItem> = {}): NightListItem => ({
  ...mockNights()[10]!, usageMinutes: 420, ahi: 1, excessLeakP95: 0,
  largeLeakMinutes: 0, interruptions: 0, settingsChanged: false,
  eventCounts: { obstructive: 7, central: 0, hypopnea: 0 }, ...changes
});
const stages: SleepStageEstimate = {
  start: "2026-11-01T22:00:00", epochSeconds: 30, stages: [],
  minutes: { wake: 0, light: 200, deep: 100, rem: 120 }, signalQuality: 0.9, estimatorVersion: 1
};

describe("synthetic score follows the contract", () => {
  it("uses component boundaries and drops missing components", () => {
    expect(computeMockScore(night(), stages)?.total).toBe(95);
    expect(computeMockScore(night({ usageMinutes: 29 }), stages)).toBeNull();
    const score = computeMockScore(night({ usageMinutes: 240, ahi: 5, largeLeakMinutes: 12, excessLeakP95: 40, interruptions: 3 }), stages)!;
    const points = Object.fromEntries(score.components.map(c => [c.key, c.points]));
    expect(points).toMatchObject({ duration: 12, breathing: 20, maskSeal: 5, continuity: 6, sleepStages: 10 });
    const missing = computeMockScore(night({ ahi: null }), null)!;
    expect(missing.components.map(c => c.key)).toEqual(["duration", "maskSeal", "continuity"]);
    expect(missing.total).toBe(95);
    expect(computeMockScore(night({ largeLeakMinutes: null }), stages)!.components.some(c => c.key === "maskSeal")).toBe(false);
    expect(computeMockScore(night({ excessLeakP95: null }), stages)!.components.find(c => c.key === "maskSeal")!.detail).toContain("extra-leak data was not recorded");
    expect(missing.components.reduce((s,c) => s + c.maxPoints, 0)).toBeCloseTo(60);
    expect(computeMockScore(night({ ahi: 30, excessLeakP95: 40, largeLeakMinutes: 84, interruptions: 8 }), stages)?.total).toBe(37);
  });
});

describe("synthetic suggestions follow contract thresholds", () => {
  it("distinguishes watch from talk for AHI and recurring high AHI", () => {
    expect(nightTips(night({ ahi: 5 }))[0]?.severity).toBe("watch");
    expect(nightTips(night({ ahi: 15 }))[0]?.severity).toBe("watch");
    expect(nightTips(night({ ahi: 15.1 }))[0]?.severity).toBe("talk");
    expect(nightTips(night({ ahi: 5 }), [night({ ahi: 5 }), night({ ahi: 6 }), night({ ahi: 5 })])[0]?.severity).toBe("talk");
    expect(nightTips(night({ ahi: 6, eventCounts: { central: 28, obstructive: 7, hypopnea: 0 } }))[0]?.id).toBe("central-watch");
  });
  it("covers strict leak/usage boundaries and mask-off breaks without settings advice", () => {
    expect(nightTips(night({ usageMinutes: 240, largeLeakMinutes: 12, excessLeakP95: 24 }))[0]?.id).toBe("all-good");
    const tips = nightTips(night({ usageMinutes: 239, largeLeakMinutes: 20, excessLeakP95: 25, interruptions: 3, settingsChanged: true }), [], true);
    expect(tips.map(t => t.id)).toEqual(["short-night", "mask-fit", "interruptions", "prescription-changed"]);
    expect(tips).toHaveLength(4);
    expect(tips.map(t => t.body).join(" ")).not.toMatch(/increase pressure|decrease pressure|change.*settings/i);
    expect(nightTips(night({ usageMinutes: 239, excessLeakP95: 25, ahi: 16, interruptions: 3, settingsChanged: true }))).toHaveLength(4);
  });
});

it("rescales unrounded points and preserves original component weights", () => {
  const score = computeMockScore(night({ usageMinutes: 420.8, ahi: 1.28, largeLeakMinutes: 420.8 * 0.0096 }), stages)!;
  expect(score.total).toBe(94);
  expect(score.components.reduce((sum, component) => sum + component.points, 0)).toBeCloseTo(93.4);
  const missing = computeMockScore(night({ ahi: null }), null)!;
  expect(missing.components.map(c => c.maxPoints)).toEqual([30, 15, 15]);
});
