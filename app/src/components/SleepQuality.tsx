import { useId } from "react";
import type { Go } from "../App";
import type { SleepScore, SleepStageEstimate, StageMinutes, Tip } from "../api/types";
import { scoreBreakdown } from "../lib/score";
import { wallSeconds, secondsToWall } from "../lib/dates";
import { hm } from "../lib/format";
import { HelpTip } from "./HelpTip";
import { Icon } from "./Icon";

export function ScoreBadge({ score, go }: { score: SleepScore | null; go?: Go }) {
  const status = !score ? "none" : score.total >= 70 ? "good" : score.total >= 50 ? "watch" : "talk";
  return <div className={`score-badge ${status}`}><Icon name={status} /><b>{score ? `${score.total}/100 · ${score.label}` : "Score unavailable"}</b><span>Estimated</span><HelpTip term="sleepScore" go={go} /></div>;
}
const COMPONENT_HELP: Record<string, string> = { duration: "hours", breathing: "ahi", maskSeal: "excessLeak", continuity: "interruptions", sleepStages: "sleepStages" };
export function ScoreBreakdown({ score, go }: { score: SleepScore | null; go?: Go }) {
  return <section className="card score-breakdown"><h2>Score breakdown <HelpTip term="sleepScore" go={go} /></h2>
    {!score ? <p className="quiet">A full score needs at least 30 minutes of use and recorded measurements. Re-read the card if night details are missing.</p> : <><p className="quiet">Points reflect this night's available data. Missing components are excluded and the total is scaled to 100.</p><ul>{scoreBreakdown(score).map((c) => <li key={c.key}><div><b>{c.label} <HelpTip term={COMPONENT_HELP[c.key] ?? "sleepScore"} go={go} /></b><span>{Math.round(c.points * 10) / 10} / {c.maxPoints}</span></div><div className="score-bar" role="meter" aria-label={c.label} aria-valuenow={c.points} aria-valuemin={0} aria-valuemax={c.maxPoints}><span style={{ width: `${c.percent}%` }} /></div><p>{c.detail}</p></li>)}</ul></>}
  </section>;
}
export const STAGE_ROWS = [
  { key: "wake", label: "Wake", help: "wake", color: "var(--watch)", pattern: "dots" },
  { key: "rem", label: "REM", help: "remSleep", color: "var(--ev2)", pattern: "diagonal" },
  { key: "light", label: "Light", help: "lightSleep", color: "var(--leak-line)", pattern: "horizontal" },
  { key: "deep", label: "Deep", help: "deepSleep", color: "var(--accent)", pattern: "cross" }
] as const;
export function StageSummary({ minutes, go }: { minutes: StageMinutes | null; go?: Go }) {
  if (!minutes) return <p className="quiet">Sleep stage estimates are not available for this range.</p>;
  const total = Object.values(minutes).reduce((a, b) => a + b, 0);
  return <dl className="stage-summary">{STAGE_ROWS.map((s) => <div key={s.key}><dt><span className={`stage-key ${s.pattern}`} style={{ backgroundColor: s.color }} />{s.label} <HelpTip term={s.help} go={go} /></dt><dd>{hm(minutes[s.key])}<small>{total > 0 ? Math.round(minutes[s.key] / total * 100) : 0}%</small></dd></div>)}</dl>;
}
export function Hypnogram({ stages, from, to, go }: { stages: SleepStageEstimate | null; from?: number; to?: number; go?: Go }) {
  const id = useId().replace(/:/g, "");
  if (!stages || !stages.stages.length) return <p className="quiet">Re-read the card to add sleep stage estimates for this night.</p>;
  const start = wallSeconds(stages.start);
  const lo = from ?? start, hi = to ?? start + stages.stages.length * stages.epochSeconds;
  const width = 640, left = 58, plot = width - left - 12;
  const x = (t: number) => left + (t - lo) / Math.max(1, hi - lo) * plot;
  const runs: { stage: string; first: number; end: number }[] = [];
  stages.stages.forEach((stage, i) => { const previous = runs[runs.length - 1]; if (previous?.stage === stage) previous.end = i + 1; else runs.push({ stage, first: i, end: i + 1 }); });
  return <div className="hypnogram"><svg viewBox={`0 0 ${width} 170`} role="img" aria-label="Estimated sleep stages over the night. Rows show wake, REM, light and deep sleep; dashed gaps mean no data.">
    <defs>{STAGE_ROWS.map((s) => <pattern key={s.key} id={`${id}-${s.key}`} width="7" height="7" patternUnits="userSpaceOnUse"><rect width="7" height="7" fill={s.color} opacity=".7" />{s.key === "wake" ? <circle cx="3" cy="3" r="1" fill="var(--surface)" /> : <path d={s.key === "light" ? "M0 3h7" : s.key === "deep" ? "M0 0l7 7M0 7L7 0" : "M0 7L7 0"} stroke="var(--surface)" strokeWidth="1" />}</pattern>)}</defs>
    {STAGE_ROWS.map((s, i) => <g key={s.key}><text x="0" y={25 + i * 30}>{s.label}</text><line x1={left} x2={width - 12} y1={20 + i * 30} y2={20 + i * 30} stroke="var(--line)" /></g>)}
    {runs.map((r, i) => { const a = Math.max(lo, start + r.first * stages.epochSeconds), b = Math.min(hi, start + r.end * stages.epochSeconds); if (a >= b) return null; const row = STAGE_ROWS.findIndex((s) => s.key === r.stage); return r.stage === "noData" ? <rect key={i} x={x(a)} y="8" width={Math.max(.5, x(b) - x(a))} height="115" fill="var(--surface-2)" stroke="var(--muted)" strokeDasharray="3 3"><title>No data</title></rect> : <rect key={i} x={x(a)} y={12 + row * 30} width={Math.max(.5, x(b) - x(a))} height="17" fill={`url(#${id}-${r.stage})`}><title>{r.stage}</title></rect>; })}
    {[0, 1, 2, 3, 4].map((i) => { const t = lo + (hi - lo) * i / 4; return <text key={i} x={x(t)} y="150" textAnchor={i === 0 ? "start" : i === 4 ? "end" : "middle"}>{secondsToWall(t).slice(11, 16)}</text>; })}
  </svg><p className="quiet">Dashed gaps: no data. <HelpTip term="sleepStages" go={go} /></p></div>;
}
const SEVERITY = { good: "Good", info: "Information", watch: "Watch", talk: "Talk to your clinician" };
export function Suggestions({ tips, go }: { tips: Tip[]; go: Go }) {
  return <section className="card suggestions"><h2>Suggestions</h2>{!tips.length ? <p className="quiet">There are no suggestions for the available data.</p> : <ul>{tips.map((tip) => <li key={tip.id}><span className={`tip-status ${tip.severity}`}><Icon name={tip.severity} />{SEVERITY[tip.severity]}</span><h3>{tip.title}</h3><p>{tip.body}</p>{tip.learnSlug ? <a href={`#learn/${tip.learnSlug}`} onClick={(e) => { e.preventDefault(); go({ screen: "learn", term: tip.learnSlug! }); }}>Learn more</a> : null}</li>)}</ul>}</section>;
}
