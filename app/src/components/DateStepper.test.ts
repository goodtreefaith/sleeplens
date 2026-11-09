import { describe, expect, it } from "vitest";
import { nightSubtitle } from "./DateStepper";

describe("night date subtitle", () => {
  it("shows both days of a completed night", () => {
    expect(nightSubtitle("2026-11-08", new Date(2026, 10, 9, 10))).toBe("Night of Sun 8 → Mon 9 Nov");
  });
  it("reserves this evening for today's evening", () => {
    expect(nightSubtitle("2026-11-09", new Date(2026, 10, 9, 20))).toBe("Night starting this evening");
    expect(nightSubtitle("2026-11-08", new Date(2026, 10, 9, 20))).not.toContain("this evening");
    expect(nightSubtitle("2026-11-09", new Date(2026, 10, 9, 10))).not.toContain("this evening");
  });
  it("keeps both months when the night crosses a month boundary", () => {
    expect(nightSubtitle("2026-10-31", new Date(2026, 10, 9, 10))).toBe("Night of Sat 31 Oct → Sun 1 Nov");
  });
});
