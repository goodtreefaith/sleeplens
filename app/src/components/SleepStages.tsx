import { useId } from "react";
import type { Go } from "../App";
import type { SleepStageEstimate, StageMinutes } from "../api/types";
import { wallSeconds, secondsToWall } from "../lib/dates";
import { useWidth } from "../lib/useWidth";
import { countSleepCycles, displayStage, displayStageMinutes, roundedStageDuration, stagePattern, stageRuns, stageShare, stageRangeStatus, TYPICAL_STAGE_RANGES, type StageMode } from "../lib/stages";
import { useStageMode } from "../lib/useStageMode";
export { roundedStageDuration } from "../lib/stages";
import { HelpTip } from "./HelpTip";
import "../stages.css";

export const STAGE_ROWS = [
  { key: "wake", label: "Awake", help: "wake", color: "var(--stage-awake)" },
  { key: "light", label: "Light", help: "lightSleep", color: "var(--stage-light)" },
  { key: "deep", label: "Deep", help: "deepSleep", color: "var(--stage-deep)" },
  { key: "rem", label: "REM", help: "remSleep", color: "var(--stage-rem)" },
  { key: "uncertain", label: "Uncertain", help: "uncertainStage", color: "var(--stage-uncertain)" }
] as const;
const SIMPLE_ROWS = [STAGE_ROWS[0], { key: "nonRem", label: "Non-REM", help: "nonRemSleep", color: "var(--stage-non-rem)" }, STAGE_ROWS[3], STAGE_ROWS[4]] as const;
function StageModeControl({ mode, change }: { mode: StageMode; change: (mode: StageMode) => void }) {
  return <div className="stage-mode-control" role="group" aria-label="Stage detail">{(["simple", "detailed"] as const).map(value => <button type="button" key={value} aria-pressed={mode === value} onClick={() => change(value)} onKeyDown={event => { if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) { event.preventDefault(); const next = event.key === "Home" ? "simple" : event.key === "End" ? "detailed" : mode === "simple" ? "detailed" : "simple"; change(next); const buttons = event.currentTarget.parentElement?.querySelectorAll("button"); buttons?.[next === "simple" ? 0 : 1]?.focus(); } }}>{value === "simple" ? "Simple" : "Detailed (experimental)"}</button>)}</div>;
}
function DetailedCaveat({ mode }: { mode: StageMode }) {
  return mode === "detailed" ? <p className="stage-caveat">Deep sleep is the least reliable part of this estimate.</p> : null;
}
const CONFOUNDER_LABEL: Record<string, string> = {
  largeLeak: "mask leak",
  artifact: "a recording artifact",
  breathingEvent: "a breathing event and the breaths after it",
  periodicBreathing: "periodic breathing"
};
export function uncertaintySummary(stages: SleepStageEstimate): string | null {
  if (!stages.minutes.uncertain || stages.minutes.uncertain < 1) return null;
  const runs = stages.uncertainRuns ?? [];
  const longest = [...runs].sort((a, b) => b.epochs - a.epochs)[0];
  const base = "Uncertain time means the breathing was recorded but could not support a stage.";
  if (!longest) return base;
  const minutes = Math.max(5, Math.round(longest.epochs * stages.epochSeconds / 60 / 5) * 5);
  return `${base} The longest stretch, about ${minutes} min, was affected by ${CONFOUNDER_LABEL[longest.reason] ?? "a confounder"}.`;
}
export const STAGE_ESTIMATE_CAPTION = "Estimated from breathing, not a sleep study. Telling sleep from wake is the most reliable part; light, deep and REM are an experiment, and deep sleep is rarely detected. Uncertain time means breathing was recorded but could not support a stage estimate.";

export function StageSummary({ minutes, stages, go, mode: override }: { minutes: StageMinutes | null; stages?: SleepStageEstimate | null; go?: Go; mode?: StageMode }) {
  const [selected, change] = useStageMode();
  const mode = override ?? selected;
  if (!minutes) return <p className="quiet">Sleep stage estimates are not available for this range.</p>;
  const cycles = stages ? countSleepCycles(stages.stages) : null;
  const values = displayStageMinutes(minutes, mode);
  const rows = mode === "simple" ? SIMPLE_ROWS : STAGE_ROWS;
  return <div className="sleep-stage-details"><StageModeControl mode={mode} change={change} /><DetailedCaveat mode={mode} /><dl className="stage-summary stage-summary-list">{rows.map(s => {
    const share = stageShare(minutes, s.key);
    const range = s.key === "uncertain" ? null : TYPICAL_STAGE_RANGES[s.key];
    return <div key={s.key}><dt><span className={`stage-key${s.key === "uncertain" ? " stage-uncertain-key" : ""}`} style={{ backgroundColor: s.color }} /><span className="stage-row-label">{s.label}</span><HelpTip term={s.help} go={go} /></dt><dd>
      <span className="stage-share-track" role="meter" aria-label={`${s.label} share.${range && s.key !== "uncertain" ? ` Typical adult range ${range[0]} to ${range[1]} percent. ${stageRangeStatus(s.key, share)}.` : " Recorded but too uncertain to assign a stage."}`} aria-valuenow={Math.round(share)} aria-valuemin={0} aria-valuemax={100}>
        <span className={`stage-share-fill${s.key === "uncertain" ? " stage-uncertain-key" : ""}`} style={{ width: `${share}%`, backgroundColor: s.color }} />
        {range && <span className="stage-typical-band" style={{ left: `${range[0]}%`, width: `${range[1] - range[0]}%` }} />}
      </span><b>{Math.round(share)}%</b><span className="stage-rounded-duration">{roundedStageDuration(values[s.key as keyof typeof values] ?? 0)}</span>
    </dd></div>;
  })}</dl><details className="stage-range-help"><summary><span className="typical-band-key" />Typical adult range <span className="stage-info-mark" aria-hidden="true">i</span></summary><p>The shaded bands show general adult ranges: awake 0–10%, {mode === "simple" ? "Non-REM 65–80%, and REM 20–25%" : "light 45–60%, deep 13–23%, and REM 20–25%"}. Sleep changes between people and nights. These breathing-based estimates cannot determine whether your sleep stages are normal. Percentages include uncertain time in the total recorded time. Uncertain time has no typical range.</p></details>
  <div className="stage-confidence"><h3>How sure is this?</h3><p className="quiet">Fixed labels for relative breathing evidence, not measured accuracy.</p><p><span>Asleep vs awake</span><b>Moderate</b><HelpTip term="sleepWakeConfidence" go={go} /></p><p><span>REM</span><b>Low</b><HelpTip term="remConfidence" go={go} /></p>{mode === "detailed" && <p><span>Deep</span><b>Very low</b><HelpTip term="deepConfidence" go={go} /></p>}</div>
  {stages ? (() => { const note = uncertaintySummary(stages); return note ? <p className="stage-uncertain-note">{note}</p> : null; })() : null}
  {stages?.periodicBreathing ? <p className="stage-uncertain-note">Breathing that waxes and wanes was seen. This pattern can have several causes, so it is not scored as REM or Deep. Mention it at your next review.</p> : null}
  {cycles !== null ? <details className="stage-cycle-help"><summary>About {cycles} sleep {cycles === 1 ? "cycle" : "cycles"} (estimated) <span className="stage-info-mark" aria-hidden="true">i</span></summary><p>Each estimated REM period counts as one cycle, with at least one cycle when any sleep is estimated. Breathing data cannot measure complete sleep cycles, so this count is only a rough guide.</p></details> : null}
  <p className="estimate-label">{STAGE_ESTIMATE_CAPTION}</p></div>;
}

export interface HypnogramScale { min: number; max: number; left: number; right: number }
export function Hypnogram({ stages, from, to, axisWidth = 70, chartScale, mode: override }: { stages: SleepStageEstimate | null; from?: number; to?: number; axisWidth?: number; chartScale?: HypnogramScale; go?: Go; mode?: StageMode }) {
  const id = useId().replace(/:/g, "");
  const [container, width] = useWidth<HTMLDivElement>(640);
  const [selected, change] = useStageMode();
  const mode = override ?? selected;
  if (!stages?.stages.length) return <p className="quiet">Re-read the card to add sleep stage estimates for this night.</p>;
  const rows = mode === "simple" ? SIMPLE_ROWS : STAGE_ROWS;
  const start = wallSeconds(stages.start);
  const lo = chartScale?.min ?? from ?? start, hi = chartScale?.max ?? to ?? start + stages.stages.length * stages.epochSeconds;
  const left = chartScale?.left ?? axisWidth, right = chartScale?.right ?? width - 12, plot = Math.max(1, right - left);
  const x = (t: number) => left + (t - lo) / Math.max(1, hi - lo) * plot;
  const runs = stageRuns(stages.stages.map(stage => displayStage(stage, mode)));
  const rowY = (stage: string) => 22 + rows.findIndex(s => s.key === stage) * 30;
  const plotHeight = rows.length * 30 - 2, axisY = plotHeight + 38, height = axisY + 16;
  const ticks = width < 400 ? 2 : 4;
  return <div className="hypnogram stepped-hypnogram" ref={container}><StageModeControl mode={mode} change={change} /><DetailedCaveat mode={mode} /><svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Estimated sleep stages through the night. Rows from top to bottom are ${rows.map(s => s.label).join(", ")}. Grey diagonal hatching means uncertain. Empty dashed outlines mean no data.`}>
    <defs><pattern id={`${id}-uncertain`} width="7" height="7" patternUnits="userSpaceOnUse"><rect width="7" height="7" fill="var(--stage-uncertain)"/><path d="M-1 1l2-2M0 7l7-7M6 8l2-2" stroke="var(--muted)" strokeWidth="1" opacity=".7"/></pattern><clipPath id={`${id}-clip`}><rect x={left} y="8" width={plot} height={plotHeight} /></clipPath></defs>
    {rows.map(s => <g key={s.key}><text x="0" y={rowY(s.key) + 4}>{s.label}</text><line x1={left} x2={right} y1={rowY(s.key)} y2={rowY(s.key)} stroke="var(--line)" strokeDasharray="2 4"/></g>)}
    <g clipPath={`url(#${id}-clip)`}>{runs.map((r, i) => {
      const a = start + r.first * stages.epochSeconds, b = start + r.end * stages.epochSeconds;
      if (b <= lo || a >= hi) return null;
      const px = x(Math.max(lo, a)), endX = x(Math.min(hi, b));
      if (stagePattern(r.stage) === "empty-dashed") return <rect key={i} x={px} y="8" width={Math.max(.5, endX - px)} height={plotHeight} fill="none" stroke="var(--muted)" strokeDasharray="3 3"><title>No data</title></rect>;
      const previous = runs[i - 1];
      const row = rows.find(s => s.key === r.stage)!;
      const uncertain = stagePattern(r.stage) === "hatched";
      return <g key={i}>{previous && previous.stage !== "noData" && previous.stage !== "uncertain" && !uncertain && a >= lo ? <line x1={x(a)} x2={x(a)} y1={rowY(previous.stage)} y2={rowY(r.stage)} stroke="var(--muted)" strokeWidth="1" opacity=".7" /> : null}<rect data-stage={r.stage} x={px} y={rowY(r.stage) - 8} width={Math.max(.5, endX - px)} height="16" rx={Math.min(4, (endX - px) / 2)} fill={uncertain ? `url(#${id}-uncertain)` : row.color} stroke="var(--stage-outline)" strokeWidth="1"><title>{`${row.label}, ${secondsToWall(Math.max(lo, a)).slice(11, 16)} to ${secondsToWall(Math.min(hi, b)).slice(11, 16)}`}</title></rect></g>;
    })}</g>
    {Array.from({ length: ticks + 1 }, (_, i) => {
      const t = lo + (hi - lo) * i / ticks;
      const endpoint = i === 0 || i === ticks;
      const cx = i === 0 ? left + 24 : i === ticks ? right - 24 : x(t);
      return <g key={i}>{endpoint ? <rect x={cx - 24} y={axisY - 17} width="48" height="25" rx="12.5" fill="var(--surface-2)" stroke="var(--line)" /> : <line x1={cx} x2={cx} y1={axisY - 21} y2={axisY - 17} stroke="var(--line-strong)"/>}<text x={cx} y={axisY} textAnchor="middle">{secondsToWall(t).slice(11, 16)}</text></g>;
    })}
  </svg><div className="hypnogram-legend" aria-label="Stage legend">{rows.map(s => <span key={s.key}><i className={`stage-key${s.key === "uncertain" ? " stage-uncertain-key" : ""}`} style={{ backgroundColor: s.color }} />{s.key === "uncertain" ? "Uncertain · hatched" : s.label}</span>)}<span><i className="stage-no-data-key"/>No data · empty dashed outline</span></div></div>;
}
