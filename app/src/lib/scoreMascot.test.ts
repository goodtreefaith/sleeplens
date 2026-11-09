import { describe, expect, it } from "vitest";
import { scoreOwlState } from "./scoreMascot";

describe("score owl artwork", () => {
  it("keeps unavailable or invalid scores in the curious state", () => {
    for (const value of [null, undefined, NaN, Infinity, -1, 101]) expect(scoreOwlState(value)).toBe("curious");
  });
  it("uses the exact decorative band boundaries without changing scores", () => {
    expect([0, 39.9, 40, 49.9, 50, 59.9, 60, 69.9, 70, 84.9, 85, 100].map(scoreOwlState))
      .toEqual(["rest", "rest", "gentle", "gentle", "steady", "steady", "bright", "bright", "happy", "happy", "celebrate", "celebrate"]);
  });
});
