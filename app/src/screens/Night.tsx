import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import type { NightDetail, TherapyEvent } from "../api/types";
import type { Go } from "../App";
import { NightCharts, type Zoom } from "../charts/NightCharts";
import { DateStepper } from "../components/DateStepper";
import { EVENT_STYLE, Icon, type MarkerKind, Swatch } from "../components/Icon";
import { HelpTip } from "../components/HelpTip";
import { ScoreBadge } from "../components/SleepQuality";
import "../night-trends-v2.css";
import { SourceTag } from "../components/Status";
import { useData, errorText } from "../lib/data";
import { wallSeconds } from "../lib/dates";
import { clock, hm, longDate, plural, r1, unitLabel } from "../lib/format";

export function NightScreen({ date, go, theme }: { date?: string; go: Go; theme: "light" | "dark" }) {
  const { nights, info } = useData();
  const d = date ?? nights[nights.length - 1]?.date;
  const [detail, setDetail] = useState<NightDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [zoom, setZoom] = useState<Zoom | null>(null);
  const [sel, setSel] = useState<number | null>(null);
  const chartCard = useRef<HTMLElement>(null);
  const has = !!d && nights.some((n) => n.date === d);

  useEffect(() => {
    if (!d || !has) { setDetail(null); return; }
    let alive = true;
    setDetail(null); setErr(null); setZoom(null); setSel(null);
    api.nightDetail(d).then((x) => alive && setDetail(x)).catch((e) => alive && setErr(errorText(e)));
    return () => { alive = false; };
  }, [d, has, info?.lastImportAt]);

  if (!d) return null;
  const unit = detail?.settings?.pressureUnit ?? info?.device?.pressureUnit ?? "unknown";
  const u = unitLabel(unit);

  return (
    <section aria-labelledby="h-night">
      <div className="screen-h">
        <h1 id="h-night" className="sr">Night detail</h1>
        <DateStepper nights={nights} date={d} onDate={(nd) => go({ screen: "night", date: nd })} />
        {detail ? <ScoreBadge score={detail.score} go={go} /> : null}
      </div>
      {!has ? (
        <div className="card placeholder"><Icon name="moon" /><h2>No therapy recorded on {longDate(d)}</h2><p>Pick another night with the arrows above.</p></div>
      ) : err ? (
        <p className="error-box" role="alert">Could not load this night: {err}</p>
      ) : !detail ? (
        <div className="card loading" role="status">Loading the night…</div>
      ) : (
        <div className="with-aside">
          <div className="stack">
            <Sessions detail={detail} />
            <section className="card" aria-labelledby="h-charts" ref={chartCard}>
              <div className="card-h"><h2 id="h-charts">Through the night</h2><p>One reading per second. Gaps are times the mask was off.</p></div>
              {detail.series ? (
                <NightCharts detail={detail} unit={unit} theme={theme} zoom={zoom} onZoom={setZoom} />
              ) : (
                <p className="quiet">The detailed recording for this night was not copied from the card. The summary on the right still applies.</p>
              )}
            </section>
          </div>
          <aside className="aside" aria-label="Night summary and events">
            <EventList detail={detail} sel={sel} onPick={(i, e) => {
              setSel(i);
              const t = wallSeconds(e.time);
              setZoom({ min: t - 240, max: t + 240 });
              // When the side panel sits below the charts (narrow window), bring the charts into view.
              const r = chartCard.current?.getBoundingClientRect();
              if (r && (r.bottom < 60 || r.top > window.innerHeight - 120)) chartCard.current?.scrollIntoView({ block: "start" });
            }} />
            <section className="card" aria-labelledby="h-sum">
              <div className="card-h"><h2 id="h-sum">Machine summary</h2></div>
              <dl className="kv">
                <dt>Hours used</dt><dd>{hm(detail.usageMinutes)}</dd>
                <dt>Interruptions <HelpTip term="interruptions" /></dt><dd>{detail.interruptions}</dd>
                <dt>Extra leak, 95th percentile <HelpTip term="excessLeak" /></dt><dd>{r1(detail.excessLeakP95)} L/min</dd>
                <dt>Large leak <HelpTip term="largeLeak" /></dt><dd>{r1(detail.largeLeakMinutes)} min</dd>
                <dt>Events per hour <HelpTip term="ahi" /></dt><dd>{detail.ahi == null ? <span className="na">not recorded</span> : r1(detail.ahi)}</dd>
                <dt>Apnea index <HelpTip term="apneaIndex" /></dt><dd>{r1(detail.summary.ai)}</dd>
                <dt>Obstructive index <HelpTip term="obstructiveApnea" /></dt><dd>{r1(detail.summary.oai)}</dd>
                <dt>Central index <HelpTip term="centralApnea" /></dt><dd>{r1(detail.summary.cai)}</dd>
                <dt>Hypopnea index <HelpTip term="hypopnea" /></dt><dd>{r1(detail.summary.hi)}</dd>
                <dt>Snore index</dt><dd>{r1(detail.summary.sni)}</dd>
                <dt>Start pressure <HelpTip term="pressure" /></dt><dd>{detail.summary.startPressure == null ? "–" : `${r1(detail.summary.startPressure)} ${u}`}</dd>
                <dt>Highest pressure <HelpTip term="pressure" /></dt><dd>{detail.summary.maxPressure == null ? "–" : `${r1(detail.summary.maxPressure)} ${u}`}</dd>
                <dt>Average pressure <HelpTip term="pressure" /></dt><dd>{r1(detail.pressureMean)} {u}</dd>
                <dt>High pressure (90%) <HelpTip term="pressureP90" /></dt><dd>{r1(detail.pressureP90)} {u}</dd>
              </dl>
              {detail.pressureSource === "computed" ? <p style={{ marginTop: 12 }}><SourceTag computed /> <span className="quiet">Pressure values</span></p> : null}
            </section>
            {detail.settings ? (
              <section className="card" aria-labelledby="h-set">
                <div className="card-h"><h2 id="h-set">Settings this night</h2>{detail.settingsChanged ? <span className="status watch"><Icon name="sliders" />Changed</span> : null}</div>
                <dl className="kv">
                  <dt>Mode</dt><dd>{detail.settings.modeLabel}</dd>
                  <dt>Pressure range <HelpTip term="pressure" /></dt><dd>{detail.settings.minPressure}–{detail.settings.maxPressure} {u}</dd>
                  <dt>Start pressure <HelpTip term="pressure" /></dt><dd>{detail.settings.startPressure} {u}</dd>
                  <dt>Ramp <HelpTip term="ramp" /></dt><dd>{detail.settings.rampMinutes ? `${detail.settings.rampMinutes} min` : "Off"}</dd>
                  <dt>Humidifier</dt><dd>Level {detail.settings.humidifierLevel}</dd>
                  <dt>Comfort <HelpTip term="comfort" /></dt><dd>Level {detail.settings.comfortLevel}</dd>
                </dl>
              </section>
            ) : null}
          </aside>
        </div>
      )}
    </section>
  );
}

function Sessions({ detail }: { detail: NightDetail }) {
  if (!detail.start || !detail.end) return null;
  const a = wallSeconds(detail.start), b = wallSeconds(detail.end);
  const span = Math.max(1, b - a);
  const pos = (t: string) => ((wallSeconds(t) - a) / span) * 100;
  // hour ticks
  const ticks: { left: number; label: string }[] = [];
  const firstHour = Math.ceil(a / 3600) * 3600;
  const step = span > 10 * 3600 ? 7200 : 3600;
  for (let t = firstHour; t <= b; t += step) ticks.push({ left: ((t - a) / span) * 100, label: new Date(t * 1000).toISOString().slice(11, 16) });
  return (
    <section className="card" aria-labelledby="h-sess">
      <div className="card-h">
        <h2 id="h-sess">Mask on</h2>
        <p>{detail.sessions.length > 1 ? `${detail.sessions.length} stretches, ${hm(detail.usageMinutes)} in total, between ${clock(detail.start)} and ${clock(detail.end)}` : `${clock(detail.start)} to ${clock(detail.end)}, ${hm(detail.usageMinutes)}`}</p>
      </div>
      <div className="timeline-bar" role="img" aria-label={detail.sessions.map((s) => `Mask on ${clock(s.start)} to ${clock(s.end)}`).join(", ")}>
        {detail.sessions.map((s, i) => (
          <span key={i} className="on" style={{ left: `${pos(s.start)}%`, width: `${Math.max(0.5, pos(s.end) - pos(s.start))}%` }} />
        ))}
      </div>
      <div className="timeline-ticks" aria-hidden="true">
        {ticks.map((t) => <span key={t.label} style={{ left: `${t.left}%` }}>{t.label}</span>)}
      </div>
    </section>
  );
}

const LIST_KINDS: MarkerKind[] = ["obstructiveApnea", "centralApnea", "hypopnea", "largeLeak"];
const SHORT_LABEL: Record<MarkerKind, string> = { obstructiveApnea: "Obstructive", centralApnea: "Central", hypopnea: "Hypopnea", largeLeak: "Large leak" };
const DEFINITION: Record<MarkerKind, string> = {
  obstructiveApnea: "Breathing stopped for 10 seconds or more because the airway narrowed or closed.",
  centralApnea: "Breathing stopped for 10 seconds or more with the airway open.",
  hypopnea: "Breathing became much shallower for 10 seconds or more.",
  largeLeak: "A lot of air escaped around the mask (over 90 L/min)."
};

function EventList({ detail, sel, onPick }: { detail: NightDetail; sel: number | null; onPick: (i: number, e: TherapyEvent) => void }) {
  const list = useMemo(() => detail.events.map((e, i) => ({ e, i })).filter(({ e }) => (LIST_KINDS as string[]).includes(e.kind)), [detail.events]);
  const notRecorded = detail.ahi == null;
  const counts = LIST_KINDS.map((k) => ({ k, n: list.filter((x) => x.e.kind === k).length })).filter((c) => !notRecorded || c.n > 0);
  return (
    <section className="card events-card" aria-labelledby="h-ev">
      <div className="card-h"><h2 id="h-ev">Events</h2><p>{notRecorded ? "Breathing events not recorded by the machine" : plural(list.length, "event")}</p></div>
      {counts.length ? (
        <div className="legend" style={{ marginBottom: 10 }}>
          {counts.map(({ k, n }) => <span key={k} title={DEFINITION[k]}><Swatch kind={k} />{EVENT_STYLE[k].short} {n} <HelpTip term={k} /></span>)}
        </div>
      ) : null}
      {list.length ? (
        <>
          <div className="evscroll">
            <div className="evcounts" role="list">
              {list.map(({ e, i }) => {
                const k = e.kind as MarkerKind;
                return (
                  <div key={i} className="event-help-row" role="listitem"><button type="button" aria-pressed={sel === i} title={DEFINITION[k]} onClick={() => onPick(i, e)}>
                    <Swatch kind={k} />
                    <span className="ev-name">{SHORT_LABEL[k]} · {e.value} {k === "largeLeak" ? "min" : "s"}</span>
                    <span className="t">{clock(e.time)}</span>
                  </button><HelpTip term={k} /></div>
                );
              })}
            </div>
          </div>
          <p className="quiet" style={{ marginTop: 10 }}>Select an event to zoom the charts to the minutes around it. <a href="#learn/ahi">What the events mean</a></p>
        </>
      ) : notRecorded ? null : <p className="quiet">No events this night.</p>}
    </section>
  );
}
