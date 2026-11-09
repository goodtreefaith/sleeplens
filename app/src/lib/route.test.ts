import { describe, expect, it } from "vitest";
import { parseHash, toHash, type Route } from "./route";

describe("Trends range navigation", () => {
  it("preserves the Overview range and its end date", () => {
    const route: Route = { screen: "trends", date: "2026-11-08", period: "month" };
    expect(parseHash(toHash(route))).toEqual(route);
  });
  it("rejects unknown ranges and invalid dates", () => {
    expect(parseHash("#trends/not-a-date?period=unknown")).toEqual({ screen: "trends" });
  });
});
