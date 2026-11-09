import { useEffect, useState } from "react";
import { api } from "../api";
import type { Trends } from "../api/types";
import type { Go } from "../App";
import { LineChart, UsageCalendar } from "../charts/TrendCharts";
import { Icon } from "../components/Icon";
import { StatusPill } from "../components/Status";
import { useData, errorText } from "../lib/data";
import { addDays, daysInclusive } from "../lib/dates";
import { hm, plural, r1, shortDateY, unitLabel } from "../lib/format";
import { ahiStatus, SETTING_LABEL } from "../lib/status";

export type RangeKey = "W" | "M" | "6M" | "Y" | "All";
const RANGE_DAYS: Record<Exclude<RangeKey, "All">, number> = { W: 7, M: 30, "6M": 183, Y: 365 };
const RANGE_LABEL: Record<RangeKey, string> = { W: "Week", M: "Month", "6M": "6 months", Y: "Year", All: "All" };
const RANGE_KEY = "sleeplens.trendsRange";

/** From/to for a range, never earlier than the first night (so compliance only counts days you had the machine). */
export function rangeDates(key: RangeKey, first: string, last: string): { from: string; to: string } {
  if (key === "All") return { from: first, to: last };
  const from = addDays(last, -(RANGE_DAYS[key] - 1));
  return { from: from < first ? first : from, to: last };
}

function readRange(): RangeKey {
  try { const v = localStorage.getItem(RANGE_KEY); return v && v in RANGE_LABEL ? (v as RangeKey) : "M"; } catch { return "M"; }
}

export function TrendsScreen({ go }: { go: Go }) {
  const { info, history } = useData();
  const [range, setRange] = useState<RangeKey>(readRange);
  const [t, setT] = useState<Trends | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const first = info?.firstDate, last = info?.lastDate;
  const unit = unitLabel(info?.device?.pressureUnit ?? history[0]?.settings.pressureUnit);

  useEffect(() => {
    if (!first || !last) return;
    let alive = true;
    api.trends(rangeDates(range, first, last)).then((x) => alive && setT(x)).catch((e) => alive && setErr(errorText(e)));
    try { localStorage.setItem(RANGE_KEY, range); } catch { /* storage unavailable */ }
    return () => { alive = false; };
  }, [range, first, last]);

  if (!first || !last) return null;
  const pick = (d: string) => go({ screen: "overview", date: d });
  const clamped = range !== "All" && t && daysInclusive(t.from, t.to) < RANGE_DAYS[range];
  const changes = history.filter((h) => h.changed.length > 0);

  return (
    <section aria-labelledby="h-trends">
      <div className="screen-h">
        <h1 id="h-trends">Trends</h1>
        <p className="sub">{t ? `${shortDateY(t.from)} to ${shortDateY(t.to)} · ${plural(t.nights, "night")} with use` : ""}{clamped ? " · since your first night" : ""}</p>
        <span className="grow" />
        <div className="seg-ctl" role="group" aria-label="Time range">
          {(Object.keys(RANGE_LABEL) as RangeKey[]).map((k) => (
            <button key={k} type="button" aria-pressed={range === k} onClick={() => setRange(k)} title={RANGE_LABEL[k]}>{k}</button>
          ))}
        </div>
      </div>
      {err ? <p className="error-box" role="alert">{err}</p> : null}
      {!t ? <div className="card loading" role="status">Loading…</div> : (
        <div className="stack">
          <div className="trend-figs">
            <div className="kpis">
              <section className="card kpi" aria-labelledby="k-comp">
                <h2 id="k-comp">Used over 4 hours</h2>
                <span className="pct">{Math.round(t.compliancePct)}%</span>
                <p className="quiet">{t.validNights} of {t.daysInRange} days</p>
                <div className="meter" role="img" aria-label={`${Math.round(t.compliancePct)} percent; guideline 70 percent`}>
                  <span style={{ width: `${Math.min(100, t.compliancePct)}%` }} />
                  <span className="g" style={{ left: "70%" }}><em>70% guide</em></span>
                </div>
                <StatusPill s={t.compliancePct >= 70 ? "good" : "watch"} word={t.compliancePct >= 70 ? "Meets the 70% guide" : "Below the 70% guide"} />
              </section>
              <section className="card kpi" aria-labelledby="k-use">
                <h2 id="k-use">Average use</h2>
                <span className="pct">{hm(t.avgUsageMinutes)}</span>
                <p className="quiet">per night with use</p>
              </section>
              <section className="card kpi" aria-labelledby="k-ahi">
                <h2 id="k-ahi">Events per hour</h2>
                <span className="pct">{r1(t.avgAhi)}</span>
                {t.avgAhi != null ? <StatusPill s={ahiStatus(t.avgAhi)} word={ahiStatus(t.avgAhi) === "good" ? "Normal range" : ahiStatus(t.avgAhi) === "watch" ? "Watch" : "Talk to your clinician"} /> : <StatusPill s="none" word="Not recorded" />}
              </section>
              <section className="card kpi" aria-labelledby="k-leak">
                <h2 id="k-leak">Extra leak, average</h2>
                <span className="pct">{r1(t.avgExcessLeak)}<small className="quiet"> L/min</small></span>
                <p className="quiet">Pressure high (90%): {r1(t.avgPressureP90)} {unit}</p>
              </section>
            </div>
            <section className="card summary" aria-labelledby="k-short">
              <h2 id="k-short">In short</h2>
              <p>
                You used it more than 4 hours on {Math.round(t.compliancePct)}% of days{t.compliancePct >= 70 ? ", above the common 70% guide" : ""}.
                {" "}Average use was {hm(t.avgUsageMinutes)} a night{t.avgAhi != null ? `, with about ${r1(t.avgAhi)} breathing events an hour` : ""}.
              </p>
              <p className="quiet" style={{ marginTop: 6 }}>Days with no recording count as not used. The 70% guide (4 hours on 70% of nights) is what many insurers use.</p>
            </section>
          </div>

          <section className="card" aria-labelledby="hm-h">
            <div className="card-h"><h2 id="hm-h">Hours each night</h2><p>One square per day. Select a night to open it.</p></div>
            <div className="heat-row">
              <UsageCalendar points={t.points} from={t.from} to={t.to} last={last} onPick={pick} />
              <ul className="heat-legend" aria-label="Legend">
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><rect x="1.5" y="1.5" width="11" height="11" rx="2" fill="none" stroke="var(--line-strong)" strokeWidth="1.5" /></svg>No recording</li>
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" rx="2" fill="url(#hatch)" /></svg>4 h or less (not a valid night)</li>
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" rx="2" fill="var(--heat1)" /></svg>4–7 h</li>
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" rx="2" fill="var(--heat2)" /></svg>7–9 h</li>
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><rect x="1" y="1" width="12" height="12" rx="2" fill="var(--heat3)" /></svg>9 h or more</li>
                <li><svg viewBox="0 0 14 14" aria-hidden="true"><circle cx="7" cy="7" r="1.6" fill="var(--line-strong)" /></svg>Outside this range or not yet</li>
              </ul>
            </div>
          </section>

          <div className="multiples">
            <section className="card mult" aria-labelledby="m-ahi">
              <h3 id="m-ahi">Events per hour (AHI)</h3>
              <p className="val">{r1(t.avgAhi)} <small>average</small></p>
              <LineChart points={t.points} from={t.from} to={t.to} value={(p) => p.ahi} guide={5} guideLabel="5 = top of normal range" unit="events/h" aria="Events per hour each night" onPick={pick} />
            </section>
            <section className="card mult" aria-labelledby="m-leak">
              <h3 id="m-leak">Extra leak (median)</h3>
              <p className="val">{r1(t.avgExcessLeak)} <small>L/min average</small></p>
              <LineChart points={t.points} from={t.from} to={t.to} value={(p) => p.excessLeakMedian} unit="L/min" aria="Median extra leak each night" onPick={pick} />
            </section>
            <section className="card mult" aria-labelledby="m-p90">
              <h3 id="m-p90">Pressure, high (90%)</h3>
              <p className="val">{r1(t.avgPressureP90)} <small>{unit} average</small></p>
              <LineChart points={t.points} from={t.from} to={t.to} value={(p) => p.pressureP90} unit={unit} aria="High pressure each night" onPick={pick} />
            </section>
          </div>

          <div className="bottom-grid">
            <section className="card" aria-labelledby="ch-h">
              <div className="card-h"><h2 id="ch-h">Settings changes</h2><p>From the machine's settings log</p></div>
              {changes.length ? (
                <ul className="changes">
                  {changes.map((c) => (
                    <li key={c.date + (c.time ?? "")}>
                      <b>{shortDateY(c.date)}{c.time ? `, ${c.time.slice(11, 16)}` : ""}</b>
                      {c.changed.filter((k) => k !== "mode").map((k) => `${SETTING_LABEL[k]}: ${String(c.settings[k])}${/Pressure$/.test(k) ? ` ${unit}` : ""}`).join(" · ")}
                    </li>
                  ))}
                </ul>
              ) : <p className="quiet">No settings changes recorded.</p>}
            </section>
            <section className="card" aria-labelledby="dn-h">
              <div className="card-h"><h2 id="dn-h">About this data</h2></div>
              <ul className="notes">
                <li className="note"><Icon name="info" /><span>A valid night is more than 4 hours of use, the same rule the machine uses.</span></li>
                <li className="note"><Icon name="info" /><span>Nights where the machine's summary log no longer has events show "not recorded", never zero, and are left out of the events average.</span></li>
                <li className="note"><Icon name="info" /><span>Dashed lines mark a settings change. Numbers before and after may not compare directly.</span></li>
              </ul>
            </section>
          </div>
        </div>
      )}
    </section>
  );
}
