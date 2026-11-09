import type { NightListItem, PressureUnit, Settings, SettingsChange } from "../api/types";
import type { Go } from "../App";
import { DateStepper, neighbours } from "../components/DateStepper";
import { Icon } from "../components/Icon";
import { RangeBar, SourceTag, StatusPill, What } from "../components/Status";
import { useData } from "../lib/data";
import { clock, hm, longDate, midDate, plural, r1, shortDate, shortDateY, unitLabel, unitSpoken } from "../lib/format";
import {
  ahiStatus, hoursStatus, largeLeakPct, leakStatus, pressureStatus, SETTING_LABEL, settingsOn, STATUS_WORD, worst,
  type Status
} from "../lib/status";

const HERO_WORD: Record<Status, string> = {
  good: "Therapy looked good.",
  watch: "Mostly fine, with something to keep an eye on.",
  talk: "Worth a look, and worth mentioning to your clinician if it keeps happening.",
  none: "Not enough data to judge."
};

export function OverviewScreen({ date, go }: { date?: string; go: Go }) {
  const { nights, history, info } = useData();
  const last = nights[nights.length - 1]?.date;
  const d = date ?? last;
  if (!d) return null;
  const n = nights.find((x) => x.date === d);
  const unit = info?.device?.pressureUnit ?? history[0]?.settings.pressureUnit ?? "unknown";
  const onDate = (nd: string) => go({ screen: "overview", date: nd });

  const head = (
    <div className="screen-h">
      <h1 id="h-overview" className="sr">Overview</h1>
      <DateStepper nights={nights} date={d} onDate={onDate} />
    </div>
  );

  if (!n) {
    const { prev, next } = neighbours(nights, d);
    return (
      <section aria-labelledby="h-overview">
        {head}
        <div className="card placeholder">
          <Icon name="moon" />
          <h2>No therapy recorded on {longDate(d)}</h2>
          <p>The card has no night for this date. The machine may not have been used.</p>
          <div className="datebar">
            {prev ? <button className="btn" type="button" onClick={() => onDate(prev)}><Icon name="chevL" />{shortDateY(prev)}</button> : null}
            {next ? <button className="btn" type="button" onClick={() => onDate(next)}>{shortDateY(next)}<Icon name="chev" /></button> : null}
          </div>
        </div>
      </section>
    );
  }

  const set = settingsOn(history, n.date);
  const change = history.find((h) => h.date === n.date && h.changed.length > 0) ?? null;
  const notes = nightNotes(n);
  return (
    <section aria-labelledby="h-overview">
      {head}
      <div className="with-aside">
        <div className="stack">
          <Hero n={n} isLatest={n.date === last} settings={set?.settings ?? null} unit={unitLabel(unit)} />
          {n.settingsChanged || change ? <SettingsChanged change={change} unit={unitLabel(unit)} /> : null}
          <div className="tiles">
            <HoursTile n={n} go={go} />
            <AhiTile n={n} go={go} />
            <LeakTile n={n} go={go} />
            <PressureTile n={n} settings={set?.settings ?? null} unit={unit} go={go} />
          </div>
        </div>
        <aside className="aside" aria-label="Night details">
          <a className="card linkcard" href={`#night/${n.date}`} onClick={(e) => { e.preventDefault(); go({ screen: "night", date: n.date }); }}>
            <Icon name="wave" />
            <div><h2>Open the night</h2><p>See when events happened, and how pressure and mask seal changed around them.</p></div>
            <Icon name="chev" />
          </a>
          {notes.length ? (
            <section className="card" aria-labelledby="a-notes">
              <div className="card-h"><h2 id="a-notes">Data notes</h2></div>
              <NoteList notes={notes} />
            </section>
          ) : null}
          <Facts n={n} settings={set?.settings ?? null} unit={unitLabel(unit)} />
        </aside>
      </div>
    </section>
  );
}

function Hero({ n, isLatest, settings, unit }: { n: NightListItem; isLatest: boolean; settings: Settings | null; unit: string }) {
  const hs = hoursStatus(n.usageMinutes);
  const as = ahiStatus(n.ahi, n.usageMinutes);
  const ls = leakStatus(n);
  const ps = pressureStatus(n, settings);
  const w = n.usageMinutes < 10 ? "none" : worst(hs, as, ls, ps === "watch" ? "good" : ps);
  const when = isLatest ? "Last night" : midDate(n.date);
  const frac = Math.min(1, n.usageMinutes / 480);
  const C = 2 * Math.PI * 32;
  const total = n.eventCounts.obstructive + n.eventCounts.central + n.eventCounts.hypopnea;
  let sub: string;
  if (n.ahi == null) sub = "The machine's summary log no longer covers this night, so there is no events count.";
  else if (n.usageMinutes < 10) sub = "Too short to judge breathing events.";
  else sub = `${plural(total, "breathing event")}, about ${r1(n.ahi)} an hour (${STATUS_WORD.ahi[as].toLowerCase()}).`;
  const seal = ls === "good" ? " The mask sealed well." : ls === "watch" ? " The mask leaked for part of the night." : ls === "talk" ? " The mask leaked for a large part of the night." : "";
  const pres = n.pressureMean != null && settings ? ` Pressure averaged ${r1(n.pressureMean)} ${unit}.` : "";
  // Nights the machine's summary no longer covers: describe what we do know, gently.
  const known = w === "good" ? "Hours and pressure looked fine" : hs === "talk" ? "A short night" : hs === "watch" ? "A bit shorter than a full night" : "Worth a look";
  const heading = n.ahi == null && n.usageMinutes >= 10
    ? `${when}: ${hm(n.usageMinutes)}. ${known}; events weren't recorded for this night.`
    : `${when}: ${hm(n.usageMinutes)}. ${HERO_WORD[w]}`;
  return (
    <section className="card headline" aria-labelledby="hl">
      <div className={`ring ${w}`} aria-hidden="true">
        <svg className="r" viewBox="0 0 76 76">
          <circle cx="38" cy="38" r="32" fill="none" stroke="var(--surface-2)" strokeWidth="7" />
          <circle cx="38" cy="38" r="32" fill="none" stroke="currentColor" strokeWidth="7" strokeLinecap="round" strokeDasharray={`${C * frac} ${C}`} />
        </svg>
        <span className="ic"><Icon name={w} /></span>
      </div>
      <div>
        <h2 id="hl">{heading}</h2>
        <p className="h-sub">{sub}{seal}{pres}</p>
        <div className="hero-status"><StatusPill s={w} word={w === "good" ? "Looks good" : w === "watch" ? "Keep an eye on it" : w === "talk" ? "Talk to your clinician if it repeats" : "Not enough data"} /></div>
      </div>
    </section>
  );
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
      <header className="tile-h"><Icon name="clock" /><h3 id="t-hours">Hours used</h3><SourceTag computed={n.usageSource === "computed"} /></header>
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
      <span>{label}</span>
      <span className="bar"><span style={{ width: `${(v / mx) * 100}%` }} /></span>
      <span className="v">{v} · {r1(hrs > 0 ? v / hrs : 0)}/h</span>
    </>
  );
  return (
    <article className="card tile" aria-labelledby="t-ahi">
      <header className="tile-h"><Icon name="lungs" /><h3 id="t-ahi">Events per hour (AHI)</h3></header>
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

function LeakTile({ n, go }: { n: NightListItem; go: Go }) {
  const s = leakStatus(n);
  const p = largeLeakPct(n);
  return (
    <article className="card tile" aria-labelledby="t-leak">
      <header className="tile-h"><Icon name="mask" /><h3 id="t-leak">Mask seal</h3></header>
      <div className="tile-v">
        <span className="big">{r1(n.excessLeakMedian)}</span><span className="unit">L/min extra leak (median)</span>
      </div>
      <StatusPill s={s} word={STATUS_WORD.leak[s]} />
      {p != null ? (
        <RangeBar
          min={0} max={30}
          aria={`Large leak for ${hm(n.largeLeakMinutes)}, ${Math.round(p)} percent of the night. Under 5 percent is good, 5 to 20 percent watch, over 20 percent talk to your clinician.`}
          zones={[{ k: "good", from: 0, to: 5, label: "<5%" }, { k: "watch", from: 5, to: 20, label: "5–20%" }, { k: "talk", from: 20, to: 30, label: "Over 20%" }]}
          markers={[{ v: Math.min(p, 30) }]}
        />
      ) : null}
      <p className="tile-sub">
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
      <header className="tile-h"><Icon name="gauge" /><h3 id="t-pres">Pressure</h3><SourceTag computed={n.pressureSource === "computed"} /></header>
      <div className="tile-v">
        <span className="big">{r1(n.pressureMean)}</span><span className="unit">{u} average</span>
        <span className="pair"><span className="big2">{r1(n.pressureP90)}</span><span className="unit">{u} high (90%)</span></span>
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

function Facts({ n, settings, unit }: { n: NightListItem; settings: Settings | null; unit: string }) {
  return (
    <section className="card" aria-labelledby="a-facts">
      <div className="card-h"><h2 id="a-facts">This night</h2></div>
      <dl className="kv">
        <dt>Started</dt><dd>{n.start ? `${clock(n.start)}${n.start.slice(0, 10) !== n.date ? `, ${shortDate(n.start.slice(0, 10))}` : ""}` : "–"}</dd>
        <dt>Ended</dt><dd>{clock(n.end)}</dd>
        {settings ? <><dt>Mode</dt><dd>{settings.modeLabel}</dd><dt>Pressure range</dt><dd>{settings.minPressure}–{settings.maxPressure} {unit}</dd><dt>Ramp</dt><dd>{settings.rampMinutes ? `${settings.rampMinutes} min` : "Off"}</dd></> : null}
        {n.ahi != null ? (
          <><dt>Obstructive apneas</dt><dd>{n.eventCounts.obstructive}</dd><dt>Central apneas</dt><dd>{n.eventCounts.central}</dd><dt>Hypopneas</dt><dd>{n.eventCounts.hypopnea}</dd></>
        ) : (
          <><dt>Breathing events</dt><dd className="na">not recorded</dd></>
        )}
      </dl>
    </section>
  );
}
