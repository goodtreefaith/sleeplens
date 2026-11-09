import { useId } from "react";
import type { Go } from "../App";
import type { SleepStageEstimate, StageMinutes } from "../api/types";
import { wallSeconds, secondsToWall } from "../lib/dates";
import { useWidth } from "../lib/useWidth";
import { countSleepCycles, stageRuns, stageShare, stageRangeStatus, TYPICAL_STAGE_RANGES } from "../lib/stages";
import { HelpTip } from "./HelpTip";
import "../stages.css";

export const STAGE_ROWS = [
  { key: "wake", label: "Awake", help: "wake", color: "var(--stage-awake)", pattern: "dots" },
  { key: "rem", label: "REM", help: "remSleep", color: "var(--stage-rem)", pattern: "diagonal" },
  { key: "light", label: "Light", help: "lightSleep", color: "var(--stage-light)", pattern: "horizontal" },
  { key: "deep", label: "Deep", help: "deepSleep", color: "var(--stage-deep)", pattern: "cross" }
] as const;

export function roundedStageDuration(minutes: number): string {
  if (minutes < 60) return `about ${Math.round(minutes / 5) * 5} min`;
  const quarters = Math.round(minutes / 15);
  const hours = Math.floor(quarters / 4);
  const fraction = ["", "¼", "½", "¾"][quarters % 4];
  return `about ${hours}${fraction ? " " + fraction : ""} h`;
}
export const STAGE_ESTIMATE_CAPTION = "Estimated from breathing, not a sleep study. Breathing-based estimates often under-count deep and REM sleep; a low number isn't by itself a sign of a problem.";

export function StageSummary({ minutes, stages, go }: { minutes: StageMinutes | null; stages?: SleepStageEstimate | null; go?: Go }) {
  if (!minutes) return <p className="quiet">Sleep stage estimates are not available for this range.</p>;
  const cycles = stages ? countSleepCycles(stages.stages) : null;
  return <div className="sleep-stage-details"><dl className="stage-summary stage-summary-list">{STAGE_ROWS.map(s => {
    const share = stageShare(minutes, s.key);
    const [low, high] = TYPICAL_STAGE_RANGES[s.key];
    return <div key={s.key}><dt><span className="stage-key" style={{ backgroundColor: s.color }} />{s.label}<HelpTip term={s.help} go={go} /></dt><dd>
      <span className="stage-share-track" role="meter" aria-label={`${s.label} share. Typical adult range ${low} to ${high} percent. ${stageRangeStatus(s.key, share)}.`} aria-valuenow={Math.round(share)} aria-valuemin={0} aria-valuemax={100}>
        <span className="stage-share-fill" style={{ width: `${share}%`, background: s.color }} />
        <span className="stage-typical-band" style={{ left: `${low}%`, width: `${high - low}%` }} />
      </span><b>{Math.round(share)}%</b><span className="stage-rounded-duration">{roundedStageDuration(minutes[s.key])}</span>
    </dd></div>;
  })}</dl><details className="stage-range-help"><summary><span className="typical-band-key" />Typical adult range <span className="stage-info-mark" aria-hidden="true">i</span></summary><p>The shaded bands show general adult ranges: awake 0–10%, REM 20–25%, light 45–60%, and deep 13–23%. Sleep changes between people and nights. These breathing-based estimates cannot determine whether your sleep stages are normal.</p></details>
  {cycles !== null ? <details className="stage-cycle-help"><summary>About {cycles} sleep {cycles === 1 ? "cycle" : "cycles"} (estimated) <span className="stage-info-mark" aria-hidden="true">i</span></summary><p>Each estimated REM period counts as one cycle, with at least one cycle when any sleep is estimated. Breathing data cannot measure complete sleep cycles, so this count is only a rough guide.</p></details> : null}
  <p className="estimate-label">{STAGE_ESTIMATE_CAPTION}</p></div>;
}

export interface HypnogramScale { min: number; max: number; left: number; right: number }

export function Hypnogram({ stages, from, to, axisWidth = 58, chartScale }: { stages: SleepStageEstimate | null; from?: number; to?: number; axisWidth?: number; chartScale?: HypnogramScale; go?: Go }) {
  const id = useId().replace(/:/g, "");
  const [container, width] = useWidth<HTMLDivElement>(640);
  if (!stages?.stages.length) return <p className="quiet">Re-read the card to add sleep stage estimates for this night.</p>;
  const start = wallSeconds(stages.start);
  const lo = chartScale?.min ?? from ?? start, hi = chartScale?.max ?? to ?? start + stages.stages.length * stages.epochSeconds;
  const left = chartScale?.left ?? axisWidth, right = chartScale?.right ?? width - 12, plot = Math.max(1, right - left);
  const x = (t: number) => left + (t - lo) / Math.max(1, hi - lo) * plot;
  const runs = stageRuns(stages.stages);
  const rowY = (stage: string) => 22 + STAGE_ROWS.findIndex(s => s.key === stage) * 30;
  const ticks = width < 400 ? 2 : 4;
  return <div className="hypnogram stepped-hypnogram" ref={container}><svg width={width} height={172} viewBox={`0 0 ${width} 172`} role="img" aria-label="Estimated sleep stages through the night. Rows from top to bottom are Awake, REM, Light and Deep. Grey hatching marks missing data.">
    <defs><pattern id={`${id}-noData`} width="7" height="7" patternUnits="userSpaceOnUse"><rect width="7" height="7" fill="var(--surface-2)"/><path d="M-1 1l2-2M0 7l7-7M6 8l2-2" stroke="var(--muted)" strokeWidth="1" opacity=".6"/></pattern><clipPath id={`${id}-clip`}><rect x={left} y="8" width={plot} height="118" /></clipPath></defs>
    {STAGE_ROWS.map(s => <g key={s.key}><text x="0" y={rowY(s.key) + 4}>{s.label}</text><line x1={left} x2={right} y1={rowY(s.key)} y2={rowY(s.key)} stroke="var(--line)" strokeDasharray="2 4"/></g>)}
    <g clipPath={`url(#${id}-clip)`}>{runs.map((r, i) => {
      const a = start + r.first * stages.epochSeconds, b = start + r.end * stages.epochSeconds;
      if (b <= lo || a >= hi) return null;
      const px = x(Math.max(lo, a)), endX = x(Math.min(hi, b));
      if (r.stage === "noData") return <rect key={i} x={px} y="8" width={Math.max(.5, endX - px)} height="118" fill={`url(#${id}-noData)`}><title>No data</title></rect>;
      const previous = runs[i - 1];
      const color = STAGE_ROWS.find(s => s.key === r.stage)!.color;
      return <g key={i}>{previous && previous.stage !== "noData" && a >= lo ? <line x1={x(a)} x2={x(a)} y1={rowY(previous.stage)} y2={rowY(r.stage)} stroke="var(--muted)" strokeWidth="1" opacity=".7" /> : null}<rect x={px} y={rowY(r.stage) - 8} width={Math.max(.5, endX - px)} height="16" rx={Math.min(4, (endX - px) / 2)} fill={color} stroke="var(--stage-outline)" strokeWidth="1"><title>{`${STAGE_ROWS.find(s => s.key === r.stage)!.label}, ${secondsToWall(Math.max(lo, a)).slice(11, 16)} to ${secondsToWall(Math.min(hi, b)).slice(11, 16)}`}</title></rect></g>;
    })}</g>
    {Array.from({ length: ticks + 1 }, (_, i) => {
      const t = lo + (hi - lo) * i / ticks;
      const endpoint = i === 0 || i === ticks;
      const cx = i === 0 ? left + 24 : i === ticks ? right - 24 : x(t);
      return <g key={i}>{endpoint ? <rect x={cx - 24} y="139" width="48" height="25" rx="12.5" fill="var(--surface-2)" stroke="var(--line)" /> : <line x1={cx} x2={cx} y1="135" y2="139" stroke="var(--line-strong)"/>}<text x={cx} y="156" textAnchor="middle">{secondsToWall(t).slice(11, 16)}</text></g>;
    })}
  </svg><div className="hypnogram-legend" aria-label="Stage legend">{STAGE_ROWS.map(s => <span key={s.key}><i className="stage-key" style={{ backgroundColor: s.color }} />{s.label}</span>)}<span><i className="stage-no-data-key"/>Grey hatched = no data</span></div></div>;
}
