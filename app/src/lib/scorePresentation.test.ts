import { describe, expect, it } from "vitest";
import type { SleepScore } from "../api/types";
import { readFileSync } from "node:fs";
import { contributorStatus, SCORE_COMPONENTS, scoreSegments } from "./scorePresentation";

const styles = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
const stageStyles = readFileSync(new URL("../stages.css", import.meta.url), "utf8");

describe("score presentation", () => {
  it("defines distinct category colours for light, system dark and manual dark themes", () => {
    const light = styles.split("@media (prefers-color-scheme: dark)")[0]!;
    const systemDark = styles.split("@media (prefers-color-scheme: dark)")[1]!.split(':root[data-theme="light"]')[0]!;
    const manualDark = styles.split(':root[data-theme="dark"]')[1]!;
    for (const theme of [light, systemDark, manualDark]) {
      const colours = SCORE_COMPONENTS.map(component => {
        expect(component.color).toMatch(/^var\(--score-[a-z-]+\)$/);
        const token = component.color.slice(4, -1);
        const colour = theme.match(new RegExp(`${token}:(#[0-9A-Fa-f]{6});`))?.[1];
        expect(colour, `${token} must exist in every theme`).toBeDefined();
        return colour;
      });
      expect(new Set(colours).size).toBe(5);
    }
  });
  it("uses each original weight and leaves five equal gaps around a complete circle", () => {
    const segments = scoreSegments(null);
    expect(segments).toHaveLength(5);
    expect(segments[0]!.length / segments[4]!.length).toBeCloseTo(3);
    expect(segments.reduce((sum, s) => sum + s.length + 4, 0)).toBeCloseTo(2 * Math.PI * 43);
    expect(segments[4]!.offset + segments[4]!.length + 4).toBeCloseTo(2 * Math.PI * 43);
    expect(segments.every(s => s.fillLength === 0)).toBe(true);
  });
  it("fills only earned points without rescaling missing component tracks", () => {
    const score: SleepScore = { total: 50, label: "Fair", estimated: true, components: [{ key: "duration", label: "Hours", points: 15, maxPoints: 30, detail: "" }] };
    const segments = scoreSegments(score);
    expect(segments[0]!.fillLength).toBeCloseTo(segments[0]!.length / 2);
    expect(segments[1]!.fillLength).toBe(0);
    expect(segments[0]!.length).toBe(scoreSegments(null)[0]!.length);
    score.components[0]!.points = 90;
    expect(scoreSegments(score)[0]!.fillLength).toBeCloseTo(segments[0]!.length);
    score.components[0]!.points = -1;
    expect(scoreSegments(score)[0]!.fillLength).toBe(0);
  });
  it("uses exact share thresholds for contributor words", () => {
    expect(contributorStatus(27, 30)).toBe("Optimal");
    expect(contributorStatus(26.9, 30)).toBe("Good");
    expect(contributorStatus(21, 30)).toBe("Good");
    expect(contributorStatus(20.9, 30)).toBe("Fair");
    expect(contributorStatus(13.5, 30)).toBe("Fair");
    expect(contributorStatus(13.4, 30)).toBe("Pay attention");
    expect(contributorStatus(NaN, 0)).toBe("Pay attention");
  });
});


describe("stage colour tokens", () => {
  it("defines four dedicated stage colours in every theme without a REM mix override", () => {
    expect(stageStyles).not.toContain("color-mix");
    for (const stage of ["awake", "rem", "light", "deep"]) {
      const colours = [...stageStyles.matchAll(new RegExp(`--stage-${stage}:(#[0-9A-Fa-f]{6})`, "g"))].map(match => match[1]!);
      expect(colours).toHaveLength(3);
      expect(colours[1]).toBe(colours[2]);
    }
  });
  it("keeps each stage fill or its outline at 3:1 against the card", () => {
    const luminance = (hex: string) => {
      const channels = [1, 3, 5].map(index => {
        const c = parseInt(hex.slice(index, index + 2), 16) / 255;
        return c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4;
      });
      return channels[0]! * .2126 + channels[1]! * .7152 + channels[2]! * .0722;
    };
    const contrast = (a: string, b: string) => {
      const la = luminance(a), lb = luminance(b);
      return (Math.max(la, lb) + .05) / (Math.min(la, lb) + .05);
    };
    const light = stageStyles.split("@media")[0]!;
    const dark = stageStyles.split(':root[data-theme="dark"]')[1]!;
    const cardLight = styles.match(/--surface:(#[0-9A-Fa-f]{6})/)![1]!;
    const cardDark = styles.split(':root[data-theme="dark"]')[1]!.match(/--surface:(#[0-9A-Fa-f]{6})/)![1]!;
    for (const [theme, background] of [[light, cardLight], [dark, cardDark]] as const) {
      const outline = theme.match(/--stage-outline:(#[0-9A-Fa-f]{6})/)?.[1];
      for (const stage of ["awake", "rem", "light", "deep"]) {
        const fill = theme.match(new RegExp(`--stage-${stage}:(#[0-9A-Fa-f]{6})`))![1]!;
        expect(Math.max(contrast(fill, background), outline ? contrast(outline, background) : 0), stage).toBeGreaterThanOrEqual(3);
      }
    }
  });
  it("keeps dark REM at least 25 lightness points apart from Light", () => {
    const dark = stageStyles.split(':root[data-theme="dark"]')[1]!;
    const lightness = (token: string) => {
      const hex = dark.match(new RegExp(`${token}:(#[0-9A-Fa-f]{6})`))![1]!;
      const channels = [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16) / 255);
      return (Math.max(...channels) + Math.min(...channels)) / 2;
    };
    expect(lightness("--stage-rem") - lightness("--stage-light")).toBeGreaterThanOrEqual(.25);
  });
});
