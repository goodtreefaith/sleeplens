import { useEffect, useState } from "react";
import { api } from "../api";
import type { PeriodSummary, StageMinutes } from "../api/types";
import { hm, shortDate, shortDateY } from "../lib/format";
import { periodTickIndices, restorativePoints } from "../lib/periodCharts";
import { useWidth } from "../lib/useWidth";
import "../period-charts.css";

type Point = { date: string; end?: string; value: number | null; deep?: number | null; rem?: number | null; pickDate?: string | null };
export function PeriodBars({ points, scores, week, onPick }: { points: Point[]; scores?: boolean; week: boolean; onPick: (date: string) => void }) {
  const [ref, width] = useWidth<HTMLDivElement>(600);
  const [active, setActive] = useState<number | null>(null);
  const left = 70, right = width - 10, top = 28, bottom = 188, height = 226;
  const max = scores ? 100 : Math.max(60, Math.ceil(Math.max(...points.map(p => p.value ?? 0)) / 60) * 60);
  const y = (v: number) => bottom - (Math.max(scores ? 40 : 0, Math.min(max, v)) - (scores ? 40 : 0)) / (max - (scores ? 40 : 0)) * (bottom - top);
  const step = (right - left) / Math.max(1, points.length), bar = Math.max(1, Math.min(42, step * .68));
  const x = (i: number) => left + (i + .5) * step;
  const labels = points.map(p => week && step < 62 ? String(Number(p.date.slice(8))) : shortDate(p.date));
  const tickIndices = periodTickIndices(labels, week, right - left);
  const point = active == null ? null : points[active];
  const describe = (p: Point) => `${shortDateY(p.date)}${p.end && p.end !== p.date ? ` to ${shortDateY(p.end)}, nightly average` : ""}: ${p.value == null ? "no estimate recorded" : scores ? `${p.value}/100` : `Deep ${hm(p.deep ?? 0)}, REM ${hm(p.rem ?? 0)}`}`;
  const ticks = scores ? [{ v: 40, label: "Poor" }, { v: 50, label: "50 Fair" }, { v: 70, label: "70 Good" }, { v: 85, label: "85 Great" }, { v: 100, label: "100" }] : [0, max / 2, max].map(v => ({ v, label: `${Number((v / 60).toFixed(1))} h` }));
  return <div className="period-bars" ref={ref}>
    <p className="period-chart-readout" aria-live="polite">{point ? describe(point) : "Use arrow keys to explore nights. Enter opens the selected night."}</p>
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} role="group" tabIndex={points.length ? 0 : undefined} aria-label={scores ? "Sleep score per night, from 40 to 100. Scores below 40 appear at the lower edge." : points.some(p => p.end !== p.date) ? "Weekly average estimated deep and REM sleep, stacked bars." : "Estimated deep and REM sleep per night, stacked bars."}
      onFocus={() => setActive(i => i ?? 0)} onBlur={() => setActive(null)} onKeyDown={e => {
        if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) { e.preventDefault(); setActive(i => e.key === "Home" ? 0 : e.key === "End" ? points.length - 1 : Math.max(0, Math.min(points.length - 1, (i ?? 0) + (e.key === "ArrowLeft" ? -1 : 1)))); }
        else if ((e.key === "Enter" || e.key === " ") && point?.value != null) { e.preventDefault(); onPick(point.pickDate ?? point.date); }
      }}>
      {ticks.map(t => <g key={t.v}><line x1={left} x2={right} y1={y(t.v)} y2={y(t.v)} stroke="var(--line-strong)" strokeDasharray="2 5" /><text x={left - 8} y={y(t.v) + 4} textAnchor="end">{t.label}</text></g>)}
      {points.map((p, i) => <g key={p.date} onMouseEnter={() => setActive(i)} onMouseLeave={() => setActive(null)} onClick={p.value == null ? undefined : () => onPick(p.pickDate ?? p.date)} className={p.value == null ? "" : "period-bar-target"}>
        <title>{describe(p)}</title>
        {p.value == null ? <line x1={x(i) - Math.min(3, bar / 2)} x2={x(i) + Math.min(3, bar / 2)} y1={bottom} y2={bottom} stroke="var(--muted)" strokeWidth="2" /> : scores ? <rect x={x(i) - bar / 2} y={Math.min(bottom - 3, y(p.value))} width={bar} height={Math.max(3, bottom - y(p.value))} rx="3" fill="var(--accent)" /> : <><rect x={x(i) - bar / 2} y={y(p.deep ?? 0)} width={bar} height={bottom - y(p.deep ?? 0)} fill="var(--stage-deep)" stroke="var(--stage-outline)" /><rect x={x(i) - bar / 2} y={y(p.value)} width={bar} height={y(p.deep ?? 0) - y(p.value)} rx="2" fill="var(--stage-rem)" stroke="var(--stage-outline)" /></>}
        {week && scores && p.value != null ? <text x={x(i)} y={Math.min(bottom - 3, y(p.value)) - 8} textAnchor="middle" className="bar-value">{p.value}</text> : null}
        {tickIndices.includes(i) ? <text x={x(i)} y={bottom + 22} textAnchor={i === 0 && !week ? "start" : i === points.length - 1 && !week ? "end" : "middle"}>{labels[i]}</text> : null}
        <rect x={left + i * step} y={top - 20} width={step} height={bottom - top + 48} fill="transparent" />
        {active === i ? <rect x={x(i) - bar / 2 - 2} y={p.value == null ? bottom - 5 : Math.min(bottom - 3, y(p.value)) - 2} width={bar + 4} height={p.value == null ? 8 : Math.max(3, bottom - y(p.value)) + 4} rx="3" fill="none" stroke="var(--ink)" strokeWidth="2" pointerEvents="none" /> : null}
      </g>)}
    </svg>
  </div>;
}
export function PeriodScoreChart({ summary, onPick }: { summary: PeriodSummary; onPick: (date: string) => void }) {
  return <PeriodBars points={summary.scoreSeries.map(p => ({ date: p.date, value: p.sleepScore }))} scores week={summary.period === "week"} onPick={onPick} />;
}
export function PeriodStagesChart({ summary, cache, onPick }: { summary: PeriodSummary; cache: Map<string, StageMinutes | null>; onPick: (date: string) => void }) {
  const [minutes, setMinutes] = useState<Map<string, StageMinutes | null>>(new Map());
  const [loading, setLoading] = useState(true);
  const [failed, setFailed] = useState(0);
  useEffect(() => {
    let active = true;
    const dates = summary.scoreSeries.filter(p => p.usageMinutes > 0).map(p => p.date);
    const result = new Map<string, StageMinutes | null>();
    let next = 0, errors = 0;
    setMinutes(result); setLoading(true); setFailed(0);
    // The existing contract exposes stages through night_detail only. Limit simultaneous reads
    // and retain only minute totals, rather than a period's worth of waveform arrays.
    async function worker() {
      while (active && next < dates.length) {
        const date = dates[next++]!;
        if (cache.has(date)) { result.set(date, cache.get(date)!); continue; }
        try { const detail = await api.nightDetail(date); const value = detail.stages?.minutes ?? null; cache.set(date, value); result.set(date, value); }
        catch { errors++; result.set(date, null); }
      }
    }
    void Promise.all([worker(), worker()]).then(() => { if (active) { setMinutes(new Map(result)); setFailed(errors); setLoading(false); } });
    return () => { active = false; };
  }, [summary, cache]);
  const points = restorativePoints(summary.from, summary.to, summary.period, minutes);
  return <>{loading ? <p className="quiet" role="status">Loading nightly stage estimates…</p> : <><p className="quiet">{summary.period === "week" || summary.period === "month" ? "One bar per night." : "Each bar is a weekly average of nights with stage estimates."} Missing estimates are gaps.{failed ? ` ${failed} night${failed === 1 ? "" : "s"} could not be loaded.` : ""}</p><PeriodBars points={points.map(p => ({ ...p, value: p.deep == null || p.rem == null ? null : p.deep + p.rem }))} week={summary.period === "week"} onPick={onPick} /><div className="restorative-legend"><span><i className="deep" />Deep</span><span><i className="rem" />REM</span></div></>}</>;
}
