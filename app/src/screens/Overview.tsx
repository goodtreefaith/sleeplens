import { useEffect, useState } from "react";
import { api } from "../api";
import type { NightDetail, Period, PeriodSummary } from "../api/types";
import { HelpTip } from "../components/HelpTip";
import { ScoreBadge, ScoreBreakdown, Hypnogram, StageSummary, Suggestions } from "../components/SleepQuality";
import { addDays } from "../lib/dates";
import { errorText } from "../lib/data";
import type { NightListItem, PressureUnit, Settings, SettingsChange } from "../api/types";
import type { Go } from "../App";
import { DateStepper } from "../components/DateStepper";
import { Icon } from "../components/Icon";
import { RangeBar, SourceTag, StatusPill, What } from "../components/Status";
import { useData } from "../lib/data";
import { clock, hm, longDate, r1, shortDate, shortDateY, unitLabel, unitSpoken } from "../lib/format";
import {
  ahiStatus, hoursStatus, largeLeakPct, leakStatus, pressureStatus, SETTING_LABEL, settingsOn, STATUS_WORD
} from "../lib/status";

const PERIODS = [{ key: "night", label: "Night" }, { key: "week", label: "Week" }, { key: "month", label: "Month" }, { key: "6months", label: "6 months" }, { key: "year", label: "Year" }] as const;
type View = "night" | Period;
const LENGTH: Record<Period, number> = { week: 7, month: 30, "6months": 182, year: 365 };
const PREVIOUS: Record<Period, string> = { week: "last week", month: "last month", "6months": "the previous 6 months", year: "last year" };
function savedView(): View { try { const v = localStorage.getItem("sleeplens-overview-view"); return PERIODS.some((p) => p.key === v) ? v as View : "night"; } catch { return "night"; } }

export function OverviewScreen({ date, go }: { date?: string; go: Go }) {
  const { nights, history, info } = useData();
  const [view, setView] = useState<View>(savedView);
  const [detail, setDetail] = useState<NightDetail | null>(null);
  const [period, setPeriod] = useState<PeriodSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const last = nights[nights.length - 1]?.date;
  const d = date ?? last;
  useEffect(() => {
    if (!d) return;
    let active = true;
    setError(null); setDetail(null); setPeriod(null);
    const load = view === "night" ? api.nightDetail(d).then((value) => { if (active) setDetail(value); }) : api.periodSummary(view, d).then((value) => { if (active) setPeriod(value); });
    void load.catch((e: unknown) => { if (active) setError(errorText(e)); });
    return () => { active = false; };
  }, [d, view, nights]);
  if (!d) return null;
  const n = nights.find((x) => x.date === d);
  const unit = info?.device?.pressureUnit ?? history[0]?.settings.pressureUnit ?? "unknown";
  const set = settingsOn(history, d);
  const change = history.find((h) => h.date === d && h.changed.length > 0) ?? null;
  const onDate = (date: string) => go({ screen: "overview", date });
  return <section className="v2-overview" aria-labelledby="h-overview">
    <div className="screen-h"><h1 id="h-overview">Overview</h1><div className="seg-ctl" role="group" aria-label="Overview period" onKeyDown={(e) => {
      const buttons = Array.from(e.currentTarget.querySelectorAll("button"));
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = e.key === "ArrowRight" ? (index + 1) % buttons.length : e.key === "ArrowLeft" ? (index + buttons.length - 1) % buttons.length : e.key === "Home" ? 0 : e.key === "End" ? buttons.length - 1 : -1;
      if (next >= 0) { e.preventDefault(); buttons[next]?.focus(); buttons[next]?.click(); }
    }}>{PERIODS.map((p) => <button key={p.key} type="button" aria-pressed={view === p.key} onClick={() => { setView(p.key); try { localStorage.setItem("sleeplens-overview-view", p.key); } catch { /* Memory-only when storage is unavailable. */ } }}>{p.label}</button>)}</div></div>
    <div className="overview-date">{view === "night" ? <DateStepper nights={nights} date={d} onDate={onDate} /> : <div className="datebar"><button className="btn icon" type="button" aria-label="Previous period" disabled={!!info?.firstDate && addDays(d, -LENGTH[view]) < info.firstDate} onClick={() => onDate(addDays(d, -LENGTH[view]))}><Icon name="chevL" /></button><strong>{shortDateY(addDays(d, 1 - LENGTH[view]))} to {shortDateY(d)}</strong><button className="btn icon" type="button" aria-label="Next period" disabled={!last || d >= last} onClick={() => onDate(addDays(d, LENGTH[view]) > last! ? last! : addDays(d, LENGTH[view]))}><Icon name="chev" /></button></div>}</div>
    {(info?.needsReread ?? 0) > 0 ? <div className="banner info reread"><Icon name="info" /><span>Re-read your card to add sleep stages and a full sleep score</span><button className="btn" type="button" onClick={() => go({ screen: "import" })}>Import</button></div> : null}
    {error ? <p className="error-box" role="alert">{error}</p> : null}
    {view !== "night" ? period ? <PeriodOverview summary={period} go={go} /> : !error ? <p className="loading" role="status">Loading period…</p> : null : !n ? <div className="card placeholder"><Icon name="moon" /><h2>No therapy recorded on {longDate(d)}</h2><p>Choose another night using the calendar.</p></div> : !detail && !error ? <p className="loading" role="status">Loading night details…</p> : <div className="overview-columns"><div className="stack">
      <section className="card score-hero"><div className={`score-ring ${!detail?.score ? "none" : detail.score.total >= 70 ? "good" : detail.score.total >= 50 ? "watch" : "talk"}`}><svg viewBox="0 0 100 100" aria-hidden="true"><circle cx="50" cy="50" r="43" fill="none" stroke="var(--surface-2)" strokeWidth="7" /><circle cx="50" cy="50" r="43" fill="none" stroke="currentColor" strokeWidth="7" strokeDasharray={`${(detail?.score?.total ?? 0) / 100 * 270.18} 270.18`} transform="rotate(-90 50 50)" /></svg><b>{detail?.score?.total ?? "–"}<small>/100</small></b></div><div><h2>Sleep score <HelpTip term="sleepScore" go={go} /></h2><ScoreBadge score={detail?.score ?? null} go={go} /><p className="h-sub">{detail?.score ? `${detail.score.label} overall, with ${hm(n.usageMinutes)} of therapy and ${n.ahi == null ? "no recorded AHI" : `${r1(n.ahi)} breathing events per hour`}.` : "A score will appear when enough night details are available."}</p><button className="btn link" type="button" onClick={() => go({ screen: "night", date: d })}>Open the night <Icon name="chev" /></button></div></section>
      <div className="tiles"><HoursTile n={n} go={go} /><AhiTile n={n} go={go} /><LeakTile n={n} go={go} /><PressureTile n={n} settings={set?.settings ?? null} unit={unit} go={go} /></div>
      <section className="card"><h2>Estimated sleep stages <HelpTip term="sleepStages" go={go} /></h2><p className="estimate-label">Estimated from breathing — not a sleep study <HelpTip term="sleepStages" go={go} /></p><Hypnogram stages={detail?.stages ?? null} go={go} /><StageSummary minutes={detail?.stages?.minutes ?? null} go={go} /></section>
    </div><aside className="stack"><ScoreBreakdown score={detail?.score ?? null} go={go} /><Suggestions tips={detail?.tips ?? []} go={go} />{n.settingsChanged || change ? <SettingsChanged change={change} unit={unitLabel(unit)} /> : null}<Facts n={n} settings={set?.settings ?? null} unit={unitLabel(unit)} go={go} />{nightNotes(n).length ? <section className="card"><h2>Data notes</h2><NoteList notes={nightNotes(n)} /></section> : null}</aside></div>}
  </section>;
}

export function periodChange(current: number | null, previous: number | null, period: Period, suffix = ""): string {
  if (current == null) return "No recorded value to compare";
  if (previous == null) return "No previous period to compare";
  const delta = Math.round((current - previous) * 10) / 10;
  return delta === 0 ? `→ unchanged from ${PREVIOUS[period]}` : `${delta > 0 ? "↑ up" : "↓ down"} ${Math.abs(delta)}${suffix} from ${PREVIOUS[period]}`;
}
function PeriodOverview({ summary: p, go }: { summary: PeriodSummary; go: Go }) {
  const metrics = [
    { name: "Average sleep score", help: "sleepScore", value: p.avgSleepScore == null ? "–" : `${Math.round(p.avgSleepScore)}/100`, change: periodChange(p.avgSleepScore, p.previous.avgSleepScore, p.period) },
    { name: "Compliance", help: "compliance", value: `${Math.round(p.compliancePct)}%`, change: periodChange(p.compliancePct, p.previous.compliancePct, p.period, " percentage points") },
    { name: "Average hours used", help: "validNight", value: hm(p.avgUsageMinutes), change: periodChange(p.avgUsageMinutes == null ? null : p.avgUsageMinutes / 60, p.previous.avgUsageMinutes == null ? null : p.previous.avgUsageMinutes / 60, p.period, " h") },
    { name: "Average AHI", help: "ahi", value: r1(p.avgAhi), change: periodChange(p.avgAhi, p.previous.avgAhi, p.period) }
  ];
  const points = p.scoreSeries;
  const x = (i: number) => 32 + i / Math.max(1, points.length - 1) * 596;
  const y = (v: number) => 138 - v / 100 * 120;
  let path = "", connected = false;
  points.forEach((point, i) => { if (point.sleepScore == null) { connected = false; return; } path += `${connected ? "L" : "M"}${x(i)} ${y(point.sleepScore)} `; connected = true; });
  return <div className="period-overview"><div className="period-kpis">{metrics.map((m) => <article className="card" key={m.name}><h2>{m.name} <HelpTip term={m.help} go={go} /></h2><p className="period-value">{m.value}</p><p className="quiet">{m.change}</p>{m.help === "compliance" ? <><p className={`tip-status ${p.compliancePct >= 70 ? "good" : "watch"}`}><Icon name={p.compliancePct >= 70 ? "good" : "watch"} />{p.compliancePct >= 70 ? "Meets the 70% guide" : "Below the 70% guide"}</p><p className="quiet">{p.validNights} valid nights <HelpTip term="validNight" go={go} /> / {p.daysInRange} calendar days</p></> : null}</article>)}</div><div className="overview-columns"><div className="stack"><section className="card"><h2>Sleep score per night <HelpTip term="sleepScore" go={go} /></h2><p className="quiet">Estimated, out of 100. Gaps mean no score was recorded.</p><svg className="period-score-chart" viewBox="0 0 640 175" role="img" aria-label={`Nightly sleep scores from ${p.from} to ${p.to}`}><title>Estimated sleep score per night</title>{[0, 50, 100].map((v) => <g key={v}><line x1="32" x2="628" y1={y(v)} y2={y(v)} stroke="var(--line)" /><text x="0" y={y(v) + 4}>{v}</text></g>)}<path d={path} fill="none" stroke="var(--accent)" strokeWidth="2" />{points.map((point, i) => point.sleepScore == null ? null : <circle key={point.date} cx={x(i)} cy={y(point.sleepScore)} r={points.length > 60 ? 1.8 : 3} fill="var(--accent)"><title>{shortDateY(point.date)}: {point.sleepScore}/100</title></circle>)}<text x="32" y="165">{shortDate(p.from)}</text><text x="628" y="165" textAnchor="end">{shortDate(p.to)}</text></svg></section><section className="card"><h2>Average estimated stage split <HelpTip term="sleepStages" go={go} /></h2><p className="estimate-label">Estimated from breathing — not a sleep study</p><StageSummary minutes={p.avgStageMinutes} go={go} /></section></div><aside className="stack"><section className="card"><h2>Nights to compare</h2>{[{ name: "Best night", night: p.bestNight }, { name: "Lowest score", night: p.worstNight }].map((entry) => <div className="compare-night" key={entry.name}><span>{entry.name}</span>{entry.night ? <button className="btn" type="button" onClick={() => go({ screen: "night", date: entry.night!.date })}>{shortDateY(entry.night.date)} <b>{entry.night.sleepScore}/100</b><Icon name="chev" /></button> : <p className="quiet">No scored nights</p>}</div>)}</section><Suggestions tips={p.tips} go={go} /></aside></div></div>;
}

function SettingsChanged({ change, unit }: { change: SettingsChange | null; unit: string }) {
  const what = change
    ? change.changed
        .filter((k) => k !== "mode")
        .map((k) => {
          const v = change.settings[k];
          const u = typeof v === "number" && /Pressure$/.test(k) ? ` ${unit}` : k === "rampMinutes" ? " min" : "";
          return `${SETTING_LABEL[k].toLowerCase()} ${String(v)}${u}`;
        })
        .join(", ")
    : "";
  return (
    <p className="banner" role="note">
      <Icon name="sliders" />
      <span><b>Settings changed this night.</b> {what ? `New ${what}.` : ""} Numbers before and after may not compare directly.</span>
    </p>
  );
}

function HoursTile({ n, go }: { n: NightListItem; go: Go }) {
  const s = hoursStatus(n.usageMinutes);
  const h = n.usageMinutes / 60;
  const t = Math.round(n.usageMinutes);
  return (
    <article className="card tile" aria-labelledby="t-hours">
      <header className="tile-h"><Icon name="clock" /><h3 id="t-hours">Hours used <HelpTip term="validNight" go={go} /></h3><SourceTag computed={n.usageSource === "computed"} /></header>
      <div className="tile-v"><span className="big">{Math.floor(t / 60)}</span><span className="unit">h</span><span className="big">{t % 60}</span><span className="unit">m</span></div>
      <StatusPill s={s} word={STATUS_WORD.hours[s]} />
      <RangeBar
        min={0} max={10}
        aria={`Hours used ${hm(n.usageMinutes)}. Under 4 hours: talk to your clinician. 4 to 7 hours: okay. 7 hours or more: good.`}
        zones={[{ k: "talk", from: 0, to: 4, label: "Under 4 h" }, { k: "watch", from: 4, to: 7, label: "4–7 h" }, { k: "good", from: 7, to: 10, label: "7 h +" }]}
        markers={[{ v: Math.min(h, 10) }]}
      />
      <p className="tile-sub">Mask on {clock(n.start)} to {clock(n.end)}.</p>
      <What learn="hours" onLearn={(t) => go({ screen: "learn", term: t })}>
        <p>Time the machine ran with the mask on, added up across the night. Many insurers look for at least 4 hours on most nights; 7 hours or more is a full night's therapy.</p>
        <p>General guide, not a target set for you.</p>
      </What>
    </article>
  );
}

function AhiTile({ n, go }: { n: NightListItem; go: Go }) {
  const s = ahiStatus(n.ahi, n.usageMinutes);
  const hrs = n.usageMinutes / 60;
  const c = n.eventCounts;
  const mx = Math.max(c.obstructive, c.central, c.hypopnea, 1);
  const row = (label: string, v: number) => (
    <>
      <span>{label} <HelpTip term={label.startsWith("Obstructive") ? "obstructiveApnea" : label.startsWith("Central") ? "centralApnea" : "hypopnea"} go={go} /></span>
      <span className="bar"><span style={{ width: `${(v / mx) * 100}%` }} /></span>
      <span className="v">{v} · {r1(hrs > 0 ? v / hrs : 0)}/h</span>
    </>
  );
  return (
    <article className="card tile" aria-labelledby="t-ahi">
      <header className="tile-h"><Icon name="lungs" /><h3 id="t-ahi">Events per hour (AHI) <HelpTip term="ahi" go={go} /></h3></header>
      {n.ahi == null ? (
        <>
          <div className="tile-v"><span className="big" aria-hidden="true">–</span><span className="sr">No value</span></div>
          <StatusPill s="none" word="Not recorded" />
          <p className="tile-sub">Not recorded by the machine for this night. This is missing data, not zero events.</p>
        </>
      ) : (
        <>
          <div className="tile-v"><span className="big">{r1(n.ahi)}</span><span className="unit">events / hour</span></div>
          <StatusPill s={s} word={n.usageMinutes < 10 ? "Too short to judge" : STATUS_WORD.ahi[s]} />
          <RangeBar
            min={0} max={25}
            aria={`AHI ${r1(n.ahi)} events per hour. Under 5 is the normal range, 5 to 15 watch, over 15 talk to your clinician.`}
            zones={[{ k: "good", from: 0, to: 5, label: "Under 5" }, { k: "watch", from: 5, to: 15, label: "5–15" }, { k: "talk", from: 15, to: 25, label: "Over 15" }]}
            markers={[{ v: n.ahi }]}
          />
          <div className="breakdown" aria-label="Breakdown by type">
            {row("Obstructive apneas", c.obstructive)}
            {row("Central apneas", c.central)}
            {row("Hypopneas", c.hypopnea)}
          </div>
        </>
      )}
      <What learn="ahi" onLearn={(t) => go({ screen: "learn", term: t })}>
        <p>AHI counts pauses in breathing (apneas) and stretches of shallow breathing (hypopneas) lasting 10 seconds or more, per hour of use. Under 5 an hour is generally considered the normal range on therapy.</p>
        <p>Obstructive means the airway narrowed or closed; central means the airway was open but the body briefly did not take a breath.</p>
      </What>
    </article>
  );
}

export function maskSealStatus(n: NightListItem) {
  const large = leakStatus(n);
  if (large === "talk") return large;
  if ((n.excessLeakP95 ?? 0) > 24 || large === "watch") return "watch";
  return n.excessLeakP95 != null ? "good" : large;
}

function LeakTile({ n, go }: { n: NightListItem; go: Go }) {
  const s = maskSealStatus(n);
  const p = largeLeakPct(n);
  return (
    <article className="card tile" aria-labelledby="t-leak">
      <header className="tile-h"><Icon name="mask" /><h3 id="t-leak">Mask seal <HelpTip term="excessLeak" go={go} /></h3></header>
      <div className="tile-v">
        <span className="big">{r1(n.excessLeakP95)}</span><span className="unit">L/min extra leak, 95th percentile <HelpTip term="excessLeak" go={go} /></span>
      </div>
      <StatusPill s={s} word={s === "watch" ? "Check mask fit" : STATUS_WORD.leak[s]} />
      {p != null ? (
        <RangeBar
          min={0} max={30}
          aria={`Large leak for ${hm(n.largeLeakMinutes)}, ${Math.round(p)} percent of the night. Under 5 percent is good, 5 to 20 percent watch, over 20 percent talk to your clinician.`}
          zones={[{ k: "good", from: 0, to: 5, label: "<5%" }, { k: "watch", from: 5, to: 20, label: "5–20%" }, { k: "talk", from: 20, to: 30, label: "Over 20%" }]}
          markers={[{ v: Math.min(p, 30) }]}
        />
      ) : null}
      <p className="tile-sub"><HelpTip term="largeLeak" go={go} />
        {n.largeLeakMinutes == null
          ? "Large-leak time was not recorded by the machine for this night."
          : n.largeLeakMinutes === 0
            ? "No large leak recorded."
            : `Large leak for ${hm(n.largeLeakMinutes)} (${Math.round(p ?? 0)}% of the night).`}
      </p>
      <What learn="leak" onLearn={(t) => go({ screen: "learn", term: t })}>
        <p>Masks have a vent that lets out the air you breathe out. "Extra leak" is air escaping beyond that vent, usually around the edge of the mask or through an open mouth.</p>
        <p>A large leak is when the machine measured a lot of escaping air (over 90 L/min). Event counts are less reliable during large leaks.</p>
      </What>
    </article>
  );
}

function PressureTile({ n, settings, unit, go }: { n: NightListItem; settings: Settings | null; unit: PressureUnit; go: Go }) {
  const s = pressureStatus(n, settings);
  const u = unitLabel(unit);
  const lo = settings ? settings.minPressure - 1 : 0;
  const hi = settings ? settings.maxPressure + 2 : 20;
  return (
    <article className="card tile" aria-labelledby="t-pres">
      <header className="tile-h"><Icon name="gauge" /><h3 id="t-pres">Pressure <HelpTip term="pressure" go={go} /></h3><SourceTag computed={n.pressureSource === "computed"} /></header>
      <div className="tile-v">
        <span className="big">{r1(n.pressureMean)}</span><span className="unit">{u} average</span>
        <span className="pair"><span className="big2">{r1(n.pressureP90)}</span><span className="unit">{u} high (P90) <HelpTip term="pressureP90" go={go} /></span></span>
      </div>
      <StatusPill s={s} word={STATUS_WORD.pressure[s]} />
      {settings && n.pressureMean != null && n.pressureP90 != null ? (
        <RangeBar
          min={lo} max={hi}
          aria={`Average pressure ${r1(n.pressureMean)}, high ${r1(n.pressureP90)} ${unitSpoken(unit)}. Your setting range is ${settings.minPressure} to ${settings.maxPressure}.`}
          zones={[{ k: "none", from: lo, to: settings.minPressure, label: "" }, { k: "band", from: settings.minPressure, to: settings.maxPressure, label: `Your setting ${settings.minPressure}–${settings.maxPressure} ${u}` }, { k: "none", from: settings.maxPressure, to: hi, label: "" }]}
          markers={[{ v: n.pressureMean, label: "avg" }, { v: n.pressureP90, label: "high", cls: "hi" }]}
        />
      ) : null}
      <p className="tile-sub">
        {settings ? `Range ${settings.minPressure}–${settings.maxPressure} ${u} (${settings.modeLabel}). ` : ""}
        {s === "watch" ? "Reached the top of the range at times. If this happens on most nights, mention it at your next visit." : s === "good" ? "Stayed inside the range the machine is allowed to use." : ""}
      </p>
      <What learn="pressure" onLearn={(t) => go({ screen: "learn", term: t })}>
        <p>An auto CPAP raises pressure when it senses the airway narrowing and lowers it when breathing is steady, within a range your clinician set.</p>
        <p>"Average" is the mean across the night. "High (90%)" is the level it stayed under 90% of the time. Pressure is shown in {u || "the machine's unit"}, the unit your machine uses{unit === "hPa" ? " (1 hPa is about 1 cmH₂O)" : ""}.</p>
      </What>
    </article>
  );
}

type Note = { warn?: boolean; text: string };

function nightNotes(n: NightListItem): Note[] {
  const notes: Note[] = [];
  if (n.usageMinutes < 10) notes.push({ warn: true, text: "Very short recording. This looks like a mask fitting or a quick test, not a night of sleep." });
  else if (n.usageMinutes < 240) notes.push({ text: `Short night (${hm(n.usageMinutes)}). Events per hour are less reliable over a few hours.` });
  if (n.ahi == null) notes.push({ text: "The machine's summary log no longer has this night, so breathing events are not recorded. Hours, seal and pressure come from the night file." });
  if (n.pressureSource === "computed" || n.usageSource === "computed") notes.push({ text: "Values marked \"Computed by SleepLens\" were worked out from the night file because the machine did not store its own summary. They may differ slightly from the machine's screen." });
  const p = largeLeakPct(n);
  if (p != null && p >= 5) notes.push({ warn: true, text: `About ${Math.round(n.largeLeakMinutes ?? 0)} minutes had a large leak. Event counts are less reliable during large leaks.` });
  if (n.settingsChanged) notes.push({ text: "Settings changed on this date. Compare with nights before it carefully." });
  return notes;
}

function NoteList({ notes }: { notes: Note[] }) {
  return (
    <ul className="notes">
      {notes.map((x, i) => (
        <li key={i} className={`note ${x.warn ? "warn" : ""}`}><Icon name={x.warn ? "watch" : "info"} /><span>{x.text}</span></li>
      ))}
    </ul>
  );
}

function Facts({ n, settings, unit, go }: { n: NightListItem; settings: Settings | null; unit: string; go: Go }) {
  return (
    <section className="card" aria-labelledby="a-facts">
      <div className="card-h"><h2 id="a-facts">This night</h2></div>
      <dl className="kv">
        <dt>Started</dt><dd>{n.start ? `${clock(n.start)}${n.start.slice(0, 10) !== n.date ? `, ${shortDate(n.start.slice(0, 10))}` : ""}` : "–"}</dd>
        <dt>Ended</dt><dd>{clock(n.end)}</dd>
        {settings ? <><dt>Mode</dt><dd>{settings.modeLabel}</dd><dt>Pressure range <HelpTip term="pressure" go={go} /></dt><dd>{settings.minPressure}–{settings.maxPressure} {unit}</dd><dt>Ramp <HelpTip term="ramp" go={go} /></dt><dd>{settings.rampMinutes ? `${settings.rampMinutes} min` : "Off"}</dd></> : null}
        {n.ahi != null ? (
          <><dt>Obstructive apneas <HelpTip term="obstructiveApnea" go={go} /></dt><dd>{n.eventCounts.obstructive}</dd><dt>Central apneas <HelpTip term="centralApnea" go={go} /></dt><dd>{n.eventCounts.central}</dd><dt>Hypopneas <HelpTip term="hypopnea" go={go} /></dt><dd>{n.eventCounts.hypopnea}</dd></>
        ) : (
          <><dt>Breathing events</dt><dd className="na">not recorded</dd></>
        )}
      </dl>
    </section>
  );
}
