import { describe, expect, it } from "vitest";
import { addDays, daysInclusive, isDate, secondsToWall, wallSeconds } from "./dates";
import { bytes, clock, hm, longDate, longDateY, r1, shortDateY, unitLabel } from "./format";
import { ahiStatus, hoursStatus, leakStatus, pressureStatus, settingsOn, worst } from "./status";
import { parseHash, toHash } from "./route";
import { rangeDates } from "../screens/Trends";

describe("format", () => {
  it("formats minutes as hours and minutes", () => {
    expect(hm(432)).toBe("7 h 12 m");
    expect(hm(45)).toBe("45 m");
    expect(hm(0.4)).toBe("under 1 m");
    expect(hm(null)).toBe("–");
  });
  it("rounds to one decimal and shows a dash for missing values", () => {
    expect(r1(1.25)).toBe("1.3");
    expect(r1(6)).toBe("6.0");
    expect(r1(null)).toBe("–");
    expect(r1(Number.NaN)).toBe("–");
  });
  it("labels pressure units from the data", () => {
    expect(unitLabel("hPa")).toBe("hPa");
    expect(unitLabel("cmH2O")).toBe("cmH₂O");
    expect(unitLabel("unknown")).toBe("");
  });
  it("reads wall-clock times without shifting by time zone", () => {
    expect(clock("2026-11-08T23:41:05")).toBe("23:41");
    expect(longDate("2026-11-08")).toBe("Sunday 8 November");
    const s = wallSeconds("2026-11-08T23:59:30");
    expect(secondsToWall(s + 60)).toBe("2026-11-09T00:00:30");
  });
  it("does date arithmetic in whole days", () => {
    expect(addDays("2026-02-27", 2)).toBe("2026-03-01");
    expect(daysInclusive("2026-11-01", "2026-11-08")).toBe(8);
  });
  it("rejects impossible calendar dates", () => {
    expect(isDate("2026-02-30")).toBe(false);
    expect(isDate("2026-13-01")).toBe(false);
    expect(isDate("2028-02-29")).toBe(true);
    expect(parseHash("#overview/2026-02-30")).toEqual({ screen: "overview" });
  });
  it("uses three-letter months and full dates with the year", () => {
    expect(shortDateY("2026-09-16")).toBe("16 Sep 2026");
    expect(longDateY("2026-11-08")).toBe("Sunday 8 November 2026");
  });
  it("formats byte sizes", () => {
    expect(bytes(512)).toBe("512 B");
    expect(bytes(1536)).toBe("1.5 KB");
    expect(bytes(85 * 1024 * 1024)).toBe("85.0 MB");
  });
});

describe("status rules (general guides)", () => {
  it("hours", () => {
    expect(hoursStatus(7 * 60)).toBe("good");
    expect(hoursStatus(5 * 60)).toBe("watch");
    expect(hoursStatus(3 * 60)).toBe("talk");
  });
  it("AHI, with missing data as none", () => {
    expect(ahiStatus(1.2)).toBe("good");
    expect(ahiStatus(5)).toBe("watch");
    expect(ahiStatus(16)).toBe("talk");
    expect(ahiStatus(null)).toBe("none");
    expect(ahiStatus(3, 5)).toBe("none");
  });
  it("leak by share of the night with a large leak", () => {
    expect(leakStatus({ largeLeakMinutes: 2, usageMinutes: 400 })).toBe("good");
    expect(leakStatus({ largeLeakMinutes: 40, usageMinutes: 400 })).toBe("watch");
    expect(leakStatus({ largeLeakMinutes: 120, usageMinutes: 400 })).toBe("talk");
    expect(leakStatus({ largeLeakMinutes: null, usageMinutes: 400 })).toBe("none");
  });
  it("pressure against the setting range", () => {
    const s = { modeLabel: "Auto CPAP", maxPressure: 8 } as never;
    expect(pressureStatus({ pressureP90: 7.2, pressureMean: 6 }, s)).toBe("good");
    expect(pressureStatus({ pressureP90: 8, pressureMean: 6 }, s)).toBe("watch");
    expect(pressureStatus({ pressureP90: 8, pressureMean: 6 }, null)).toBe("none");
  });
  it("worst ignores none", () => {
    expect(worst("good", "none", "watch")).toBe("watch");
    expect(worst("none", "none")).toBe("none");
  });
  it("finds the settings in effect on a date", () => {
    const h = [{ date: "2026-07-07", v: 1 }, { date: "2026-09-15", v: 2 }];
    expect(settingsOn(h, "2026-08-01")?.v).toBe(1);
    expect(settingsOn(h, "2026-09-15")?.v).toBe(2);
    expect(settingsOn(h, "2026-01-01")?.v).toBe(1);
  });
});

describe("routing and ranges", () => {
  it("round-trips hashes", () => {
    expect(parseHash("#night/2026-11-08")).toEqual({ screen: "night", date: "2026-11-08" });
    expect(parseHash("#learn/ahi")).toEqual({ screen: "learn", term: "ahi" });
    expect(parseHash("#nonsense")).toEqual({ screen: "overview" });
    expect(toHash({ screen: "overview", date: "2026-11-08" })).toBe("#overview/2026-11-08");
  });
  it("clamps trend ranges to the first night", () => {
    expect(rangeDates("W", "2026-07-08", "2026-11-08")).toEqual({ from: "2026-11-02", to: "2026-11-08" });
    expect(rangeDates("Y", "2026-07-08", "2026-11-08")).toEqual({ from: "2026-07-08", to: "2026-11-08" });
    expect(rangeDates("All", "2026-07-08", "2026-11-08").from).toBe("2026-07-08");
  });
});
