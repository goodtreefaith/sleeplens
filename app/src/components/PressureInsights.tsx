import type { Go } from "../App";
import type { PressureSummary, PressureUnit, Tip } from "../api/types";
import { r1, shortDateY, unitLabel } from "../lib/format";
import { HelpTip } from "./HelpTip";
import { Icon } from "./Icon";
import "../pressure-insights.css";

const SEVERITY = { good: "Good", info: "Information", watch: "Watch", talk: "Talk to your clinician" };

export function PressureInsights({ tips, summary, nights, unit = "unknown", go }: { tips: Tip[]; summary?: PressureSummary | null; nights?: number; unit?: PressureUnit; go: Go }) {
  const pressureTips = tips.filter(tip => tip.category === "pressure");
  const u = unitLabel(unit);
  return <section className="card pressure-insights" aria-label="Pressure insights">
    <h2>Pressure insights <HelpTip term="pressureInsights" go={go} /></h2>
    {summary ? <ul className="pressure-facts">
      <li>Prescribed range {summary.settingMin == null || summary.settingMax == null ? "not recorded" : `${summary.settingMin}–${summary.settingMax} ${u}`} <HelpTip term="pressureRange" go={go} /></li>
      <li>P90 averaged {summary.avgP90 == null ? "not recorded" : `${r1(summary.avgP90)} ${u}`} <HelpTip term="pressureP90" go={go} /></li>
      <li>Near the maximum on {summary.nightsNearMax} of {nights ?? 0} nights <HelpTip term="pressureNearMax" go={go} /></li>
      <li>Time near max: {summary.avgShareAtMaxPct == null ? "not recorded" : `${r1(summary.avgShareAtMaxPct)}%`} <HelpTip term="pressureTimeNearMax" go={go} /></li>
      <li>Events at the minimum pressure: {summary.eventsAtMinPct == null ? "not recorded" : `${r1(summary.eventsAtMinPct)}%`} <HelpTip term="pressureEventsAtMin" go={go} /></li>
    </ul> : null}
    {pressureTips.length ? <ul className="pressure-tip-list">{pressureTips.map(tip => <li key={tip.id}>
      <span className={`tip-status ${tip.severity}`}><Icon name={tip.severity} />{SEVERITY[tip.severity]}</span>
      <h3>{tip.title}</h3><p>{tip.body}</p>
      {tip.evidence ? <p className="tip-evidence">{tip.evidence}</p> : null}
      {tip.evidenceDates.length ? <div className="evidence-dates" aria-label="Nights supporting this pressure insight">{tip.evidenceDates.map(date => <a key={date} href={`#night/${date}`} onClick={event => { event.preventDefault(); go({ screen: "night", date }); }}>{shortDateY(date)}</a>)}</div> : null}
      {tip.learnSlug ? <a href={`#learn/${tip.learnSlug}`} onClick={event => { event.preventDefault(); go({ screen: "learn", term: tip.learnSlug! }); }}>Learn more ›</a> : null}
    </li>)}</ul> : <p className="quiet">{nights == null ? "Nothing to flag about pressure for this night." : nights < 7 ? "Not enough nights yet to look for pressure patterns (needs at least 7)." : "Nothing to flag about pressure in this period."}</p>}
    <p className="pressure-disclaimer">SleepLens never changes settings. Only your clinician should change your pressure.</p>
  </section>;
}
