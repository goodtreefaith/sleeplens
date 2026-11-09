import { describe, expect, it } from "vitest";
import { advanceImportMonths, finishImportMonths } from "./importMonths";

describe("import month progress", () => {
  it("retains observed month transitions, ending only after completion", () => {
    const jan = advanceImportMonths({}, { phase: "nights", done: 1, total: 40, currentDate: "2026-01-20" });
    expect(jan).toEqual({ "2026-01": "Reading…" });
    const feb = advanceImportMonths(jan, { phase: "nights", done: 25, total: 40, currentDate: "2026-02-01" });
    expect(feb).toEqual({ "2026-01": "Done", "2026-02": "Reading…" });
    expect(advanceImportMonths(feb, { phase: "done", done: 40, total: 40 })).toEqual({ "2026-01": "Done", "2026-02": "Done" });
  });
  it("does not call partially skipped months already imported", () => {
    const months = { "2026-01": "Reading…" } as const;
    const result = { importId: "mock", status: "ok", nightsAdded: 1, nightsUpdated: 0, nightsSkipped: 20, nightsFailed: [] } as const;
    expect(finishImportMonths(months, { ...result, nightsFailed: [] })["2026-01"]).toBe("Done");
    expect(finishImportMonths(months, { ...result, nightsAdded: 0, nightsFailed: [] })["2026-01"]).toBe("Skipped — already imported");
    expect(finishImportMonths(months, { ...result, status: "cancelled", nightsFailed: [] })["2026-01"]).toBe("Reading…");
  });
});
