import { describe, expect, it } from "vitest";
import { periodDelta } from "./periodDelta";

describe("period deltas", () => {
  it("calls falling AHI better and rising AHI worse", () => {
    expect(periodDelta(2, 5, "month", " events/h", true)).toMatchObject({ tone: "better", icon: "↓" });
    expect(periodDelta(5, 2, "month", " events/h", true).text).toContain("worse");
  });
  it("calls higher scores and usage better", () => {
    expect(periodDelta(90, 80, "month").tone).toBe("better");
    expect(periodDelta(5, 6, "week").tone).toBe("worse");
  });
  it("rounds score changes to whole points", () => {
    expect(periodDelta(91.7, 89.4, "month", " points", false, 0).text).toContain("2 points");
    expect(periodDelta(91.4, 91.2, "month", " points", false, 0).text).toContain("About the same");
  });
  it("does not invent a change when the previous period is absent", () => {
    expect(periodDelta(80, null, "month")).toMatchObject({ tone: "neutral", text: "No previous period to compare" });
  });
});
