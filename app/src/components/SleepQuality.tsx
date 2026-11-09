import type { Go } from "../App";
import type { SleepScore, Tip } from "../api/types";
import { scoreBreakdown } from "../lib/score";
import { contributorStatus, scoreSegments, type ScoreComponentKey } from "../lib/scorePresentation";
import "../score-import.css";
import { HelpTip } from "./HelpTip";
import { Icon } from "./Icon";

export function ScoreBadge({ score, go }: { score: SleepScore | null; go?: Go }) {
  const status = !score ? "none" : score.total >= 70 ? "good" : score.total >= 50 ? "watch" : "talk";
  return <div className={`score-badge ${status}`}><Icon name={status} /><b>{score ? `${score.total}/100 · ${score.label}` : "Score unavailable"}</b><span className="estimated-tag">Estimated</span><HelpTip term="sleepScore" go={go} /></div>;
}
const COMPONENT_HELP: Record<string, string> = { duration: "hours", breathing: "ahi", maskSeal: "excessLeak", continuity: "interruptions", sleepStages: "sleepStages" };
export function ScoreBreakdown({ score, go }: { score: SleepScore | null; go?: Go }) {
  return <section className="card score-breakdown"><HelpTip term="sleepScore" go={go} variant="info" /><h2>Score breakdown <HelpTip term="sleepScore" go={go} /></h2>
    {!score ? <p className="quiet">A full score needs at least 30 minutes of use and recorded measurements. Re-read the card if night details are missing.</p> : <><p className="quiet">Points reflect this night's available data. Missing components are excluded and the total is scaled to 100.</p><ul>{scoreBreakdown(score).map((c) => <li key={c.key}><div><b>{c.key === "sleepStages" ? "Sleep stages · estimated (10 points)" : c.label} <HelpTip term={COMPONENT_HELP[c.key] ?? "sleepScore"} go={go} /></b><span>{Math.round(c.points * 10) / 10} / {c.maxPoints}</span></div><div className="score-bar" role="meter" aria-label={c.label} aria-valuenow={c.points} aria-valuemin={0} aria-valuemax={c.maxPoints}><span style={{ width: `${c.percent}%` }} /></div><p>{c.detail}</p></li>)}</ul></>}
  </section>;
}
export function ScoreCard({ score, go, summary, onContributor }: { score: SleepScore | null; go: Go; summary: string; onContributor?: (key: ScoreComponentKey) => void }) {
  const segments = scoreSegments(score);
  return <section className="card score-contributors" aria-labelledby="score-card-title">
    <HelpTip term="sleepScore" go={go} variant="info" />
    <div className="score-card-intro">
      <div className="score-ring segmented-score">
        <svg viewBox="0 0 100 100" aria-hidden="true">{segments.map(segment => <g key={segment.key} transform="rotate(-90 50 50)">
          <circle cx="50" cy="50" r="43" fill="none" stroke="var(--line-strong)" strokeWidth="7" strokeDasharray={`${segment.length} ${segment.circumference}`} strokeDashoffset={-segment.offset} />
          {segment.fillLength > 0 && <circle cx="50" cy="50" r="43" fill="none" stroke={segment.color} strokeWidth="7" strokeDasharray={`${segment.fillLength} ${segment.circumference}`} strokeDashoffset={-segment.offset} />}
        </g>)}</svg>
        <b>{score?.total ?? "–"}<small>/100</small></b>
      </div>
      <div><h2 id="score-card-title">Sleep score</h2><p className="score-verdict">{score?.label ?? "Score unavailable"}</p><p className="score-estimated">Estimated <HelpTip term="sleepScore" go={go} /></p></div>
    </div>
    <p className="score-card-summary">{summary}</p>
    <ul className="score-contributor-list">{segments.map(segment => {
      const component = segment.component;
      const status = component ? contributorStatus(component.points, component.maxPoints) : "No data";
      const meter = !component ? 0 : segment.share >= .9 ? 3 : segment.share >= .45 ? 2 : 1;
      return <li key={segment.key}><button type="button" className="score-contributor" onClick={() => onContributor ? onContributor(segment.key) : go({ screen: "learn", term: COMPONENT_HELP[segment.key]! })}>
        <span className="score-contributor-heading"><span className="score-component-dot" style={{ background: segment.color }} /><strong>{segment.label}</strong><span className="score-contributor-points">{component ? `${Math.round(component.points * 10) / 10} / ${component.maxPoints}` : `– / ${segment.maxPoints}`}</span></span>
        <span className="score-contributor-reading"><span className="score-three-step" aria-hidden="true">{[1, 2, 3].map(step => <i key={step} data-filled={step <= meter} style={step <= meter ? { background: segment.color, borderColor: segment.color } : undefined} />)}</span><span className="score-contributor-status">{status}</span><Icon name="chev" /></span>
        <span className="score-contributor-detail">{component?.detail ?? "Not recorded. This part is excluded from the total."}</span>
      </button></li>;
    })}</ul>
    <p className="score-meter-legend"><span className="score-three-step" aria-hidden="true"><i /><i /><i /></span>Meter: Poor / Sufficient / Optimal</p>
    <p className="quiet score-missing-note">Missing parts are excluded; available points are scaled to 100.</p>
  </section>;
}
export { STAGE_ROWS, roundedStageDuration, STAGE_ESTIMATE_CAPTION, StageSummary, Hypnogram } from "./SleepStages";
const SEVERITY = { good: "Good", info: "Information", watch: "Watch", talk: "Talk to your clinician" };
export function Suggestions({ tips, go }: { tips: Tip[]; go: Go }) {
  return <section className="card suggestions"><h2>Suggestions</h2>{!tips.length ? <p className="quiet">There are no suggestions for the available data.</p> : <ul>{tips.map((tip) => <li key={tip.id}><span className={`tip-status ${tip.severity}`}><Icon name={tip.severity} />{SEVERITY[tip.severity]}</span><h3>{tip.title}</h3><p>{tip.body}</p>{tip.evidence ? <p className="tip-evidence">{tip.evidence}</p> : null}{tip.evidenceDates.length ? <div className="evidence-dates" aria-label="Nights supporting this suggestion">{tip.evidenceDates.map(date => <a key={date} href={`#night/${date}`} onClick={e => { e.preventDefault(); go({ screen: "night", date }); }}>{date}</a>)}</div> : null}{tip.learnSlug ? <a href={`#learn/${tip.learnSlug}`} onClick={(e) => { e.preventDefault(); go({ screen: "learn", term: tip.learnSlug! }); }}>Learn more ›</a> : null}</li>)}</ul>}</section>;
}
