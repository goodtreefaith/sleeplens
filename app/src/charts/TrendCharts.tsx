import { useState } from "react";
import type { TrendPoint } from "../api/types";
import { addDays, dateMs, daysInclusive, DAY_MS } from "../lib/dates";
import { hm, monthYear, shortDate, shortDateY } from "../lib/format";
import { useWidth } from "../lib/useWidth";

/** Nightly values with range bands, score guides and prescription-change markers. */
export function LineChart({ points, from, to, value, typicalMax, typicalLabel, scoreGuides = false, unit, aria, onPick, changeDescriptions = {}, max }: {
  points: TrendPoint[]; from: string; to: string;
  value: (p: TrendPoint) => number | null;
  typicalMax?: number; typicalLabel?: string; scoreGuides?: boolean; changeDescriptions?: Record<string, string>; max?: number; unit: string; aria: string;
  onPick: (d: string) => void;
}) {
  const [ref, w] = useWidth<HTMLDivElement>();
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const [activeMarker, setActiveMarker] = useState<string | null>(null);
  const h = 170, padL = scoreGuides ? 70 : 36, padR = 8, padT = 24, padB = 24;
  const vals = points.map(value).filter((v): v is number => v != null);
  const ticksY = niceTicks(max ?? Math.max(typicalMax ?? 0, ...vals, 1) * 1.15);
  const maxV = ticksY[ticksY.length - 1]!;
  const minV = scoreGuides ? 40 : 0;
  const scoreTicks = [{ v: 40, label: "Poor" }, { v: 50, label: "50 Fair" }, { v: 70, label: "70 Good" }, { v: 85, label: "85 Great" }, { v: 100, label: "100" }];
  const x0 = dateMs(from), x1 = dateMs(to) + DAY_MS;
  const sx = (d: string) => padL + ((dateMs(d) + DAY_MS / 2 - x0) / (x1 - x0)) * (w - padL - padR);
  const sy = (v: number) => padT + (1 - (v - minV) / (maxV - minV)) * (h - padT - padB);
  // Break the line where a day has no value.
  const segs: string[] = [];
  let cur = "";
  let prevDate: string | null = null;
  for (const p of points) {
    const v = value(p);
    if (v == null || (prevDate && daysInclusive(prevDate, p.date) > 2)) { if (cur) segs.push(cur); cur = ""; }
    if (v != null) cur += `${cur ? "L" : "M"}${sx(p.date).toFixed(1)} ${sy(Math.max(minV, v)).toFixed(1)}`;
    prevDate = p.date;
  }
  if (cur) segs.push(cur);
  const xt = dateTicks(from, to, w);
  const changes = points.filter((p) => p.prescriptionChanged);
  const dots = points.length <= 40;
  const active = activeIndex == null ? null : points[Math.min(activeIndex, points.length - 1)];
  const describe = (p: TrendPoint) => `${shortDateY(p.date)}: ${value(p) == null ? "not recorded" : `${value(p)!.toFixed(1)} ${unit}`}${p.prescriptionChanged ? `. ${changeDescriptions[p.date] || "Mode or pressure prescription changed"}` : ""}`;
  return (
    <div className="chartwrap" ref={ref}>
      <p className="chart-guide-label">{typicalLabel ? `General guide: ${typicalLabel}` : "\u00a0"}</p>
      <p className="chart-point-readout" aria-live="polite">{active ? describe(active) : "Use arrow keys to explore nights. Enter opens the night."}</p>
      {activeMarker ? <p className="chart-marker-tooltip" role="status">{activeMarker}</p> : null}
      <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} role="group" tabIndex={points.length ? 0 : undefined} aria-label={`${aria}.${scoreGuides ? " Scale from 40 to 100; scores below 40 appear at the lower edge." : ""} Arrow keys select a night; Enter opens it.`}
        onFocus={() => setActiveIndex((i) => i ?? 0)} onBlur={() => setActiveIndex(null)}
        onKeyDown={(e) => {
          if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) {
            e.preventDefault();
            setActiveIndex((i) => e.key === "Home" ? 0 : e.key === "End" ? points.length - 1 : Math.max(0, Math.min(points.length - 1, (i ?? 0) + (["ArrowLeft", "ArrowDown"].includes(e.key) ? -1 : 1))));
          } else if ((e.key === "Enter" || e.key === " ") && active) { e.preventDefault(); onPick(active.date); }
        }}>
        {typicalMax != null ? <rect className="typical-range-band" x={padL} y={sy(typicalMax)} width={w - padL - padR} height={sy(minV) - sy(typicalMax)} /> : null}
        {!scoreGuides ? ticksY.map((t) => (
          <g key={t}>
            <line className="gridline" x1={padL} x2={w - padR} y1={sy(t)} y2={sy(t)} />
            <text className="tick" x={padL - 6} y={sy(t) + 4} textAnchor="end">{t}</text>
          </g>
        )) : scoreTicks.map((t) => <g key={t.v}>
          <line className="score-guide" x1={padL} x2={w - padR} y1={sy(t.v)} y2={sy(t.v)} />
          <text className="tick" x={padL - 8} y={sy(t.v) + 4} textAnchor="end">{t.label}</text>
        </g>)}
        {segs.map((d, i) => <path key={i} className="lineplot" d={d} />)}
        {dots ? points.map((p) => { const v = value(p); return v == null ? null : <circle key={p.date} cx={sx(p.date)} cy={sy(Math.max(minV, v))} r={2.5} fill="var(--accent)" />; }) : null}
        {active && value(active) != null ? <circle className="focused-point" cx={sx(active.date)} cy={sy(Math.max(minV, value(active)!))} r={5} fill="var(--surface)" stroke="var(--accent)" strokeWidth={3} /> : null}
        {xt.map((t) => <text key={t} className="tick" x={sx(t)} y={h - 6} textAnchor="middle">{shortDate(t)}</text>)}
        {points.map((p) => {
          const v = value(p);
          return (
            <rect key={"h" + p.date} x={sx(p.date) - 4} y={padT} width={8} height={h - padT - padB} fill="transparent" style={{ cursor: "pointer" }} onClick={() => onPick(p.date)} onMouseEnter={() => setActiveIndex(points.indexOf(p))} onMouseLeave={() => setActiveIndex(null)} aria-label={`${shortDateY(p.date)}, ${v == null ? "not recorded" : `${v.toFixed(1)} ${unit}`}. Open night`} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onPick(p.date); } }}>
              <title>{`${shortDateY(p.date)}: ${v == null ? "not recorded" : `${v.toFixed(1)} ${unit}`}`}</title>
            </rect>
          );
        })}
        {changes.map((c) => {
          const text = `${shortDateY(c.date)}: ${changeDescriptions[c.date] || "Mode or pressure prescription changed"}`;
          return <g key={"change" + c.date} className="prescription-marker" role="img" aria-label={text} onFocus={() => setActiveMarker(text)} onBlur={() => setActiveMarker(null)} onMouseEnter={() => setActiveMarker(text)} onMouseLeave={() => setActiveMarker(null)} onKeyDown={(e) => { if (e.key === "Escape") setActiveMarker(null); }}>
            <title>{text}</title>
            <line className="mk" x1={sx(c.date)} x2={sx(c.date)} y1={padT - 6} y2={h - padB} />
            <line x1={sx(c.date)} x2={sx(c.date)} y1={padT - 6} y2={h - padB} stroke="transparent" strokeWidth={10} />
          </g>;
        })}
      </svg>
    </div>
  );
}

export function niceTicks(max: number): number[] {
  const step = max <= 3 ? 1 : max <= 8 ? 2 : max <= 20 ? 5 : max <= 50 ? 10 : 20;
  const out: number[] = [];
  for (let v = 0; v <= Math.ceil(max / step) * step; v += step) out.push(v);
  return out;
}

function dateTicks(from: string, to: string, w: number): string[] {
  const days = daysInclusive(from, to);
  const want = Math.max(2, Math.floor(w / 90));
  const step = Math.max(1, Math.ceil(days / want));
  const out: string[] = [];
  for (let i = 0; i < days; i += step) out.push(addDays(from, i));
  return out;
}

/** Usage calendar: one small month grid per month, a square per day, Monday first. */
export function UsageCalendar({ points, from, to, last, onPick }: { points: TrendPoint[]; from: string; to: string; last: string; onPick: (d: string) => void }) {
  const [calendarRef, calendarWidth] = useWidth<HTMLDivElement>();
  const [focusDate, setFocusDate] = useState(from);
  const [focusedLabel, setFocusedLabel] = useState("");
  const currentFocus = focusDate < from || focusDate > to ? from : focusDate;
  const by = new Map(points.map((p) => [p.date, p]));
  const months: string[] = [];
  for (let d = from.slice(0, 7) + "-01"; d <= to; ) {
    months.push(d.slice(0, 7));
    const [y, m] = d.split("-").map(Number) as [number, number];
    d = m === 12 ? `${y + 1}-01-01` : `${y}-${String(m + 1).padStart(2, "0")}-01`;
  }
  const cell = 17, gap = 3, step = cell + gap;
  const capacity = Math.max(1, Math.floor((calendarWidth + 24) / 158));
  const rows = Math.max(1, Math.ceil(months.length / capacity));
  const columns = Math.ceil(months.length / rows);
  const WD_LABEL: Record<number, string> = { 0: "M", 2: "W", 4: "F" };
  return (
    <div className="heat-months heat balanced-months" ref={calendarRef} style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 138px))` }}>
      <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden="true">
        <defs>
          <pattern id="hatch" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="5" height="5" fill="var(--heat-low)" opacity=".35" />
            <line x1="0" y1="0" x2="0" y2="5" stroke="var(--heat-low)" strokeWidth="2.5" />
          </pattern>
        </defs>
      </svg>
      <p className="heat-focus-readout quiet" aria-live="polite">{focusedLabel || "Use arrow keys to explore days. Enter opens a recorded night."}</p>
      {months.map((m) => {
        const first = `${m}-01`;
        const [y, mo] = m.split("-").map(Number) as [number, number];
        const daysIn = new Date(Date.UTC(y, mo, 0)).getUTCDate();
        const lead = (new Date(dateMs(first)).getUTCDay() + 6) % 7;
        const weeks = Math.ceil((lead + daysIn) / 7);
        const W = weeks * step, H = 7 * step;
        let valid = 0, recorded = 0;
        const cells = [];
        for (let i = 0; i < daysIn; i++) {
          const d = `${m}-${String(i + 1).padStart(2, "0")}`;
          const k = lead + i;
          const cx = Math.floor(k / 7) * step, cy = (k % 7) * step;
          const p = by.get(d);
          if (d < from || d > to || d > last) {
            // Outside the range, or after the last imported night: a faint dot, not an empty box.
            cells.push(<circle key={d} cx={cx + cell / 2} cy={cy + cell / 2} r={1.6} fill="var(--line-strong)"><title>{`${shortDateY(d)}: ${d > last ? "not imported yet" : "outside this range"}`}</title></circle>);
            continue;
          }
          if (p && p.usageMinutes > 0) recorded++;
          if (p && p.usageMinutes > 240) valid++;
          const h = p ? p.usageMinutes / 60 : 0;
          const fill = !p || p.usageMinutes <= 0 ? undefined : h <= 4 ? "url(#hatch)" : h < 7 ? "var(--heat1)" : h < 9 ? "var(--heat2)" : "var(--heat3)";
          const label = `${shortDateY(d)}: ${p ? hm(p.usageMinutes) : "no recording"}`;
          cells.push(
            <rect
              key={d}
              className={`c${fill ? "" : " nodata"}`}
              x={fill ? cx : cx + 0.75} y={fill ? cy : cy + 0.75} width={fill ? cell : cell - 1.5} height={fill ? cell : cell - 1.5} rx={3}
              fill={fill}
              data-day={d}
              tabIndex={d === currentFocus ? 0 : -1}
              role="button" aria-disabled={!p}
              aria-label={p ? `${label}. Open this night` : label}
              onFocus={() => { setFocusDate(d); setFocusedLabel(label); }}
              onBlur={() => setFocusedLabel("")}
              onClick={p ? () => onPick(d) : undefined}
              onKeyDown={(e) => {
                const steps: Record<string, number> = { ArrowUp: -1, ArrowDown: 1, ArrowLeft: -7, ArrowRight: 7 };
                if (e.key in steps || e.key === "Home" || e.key === "End") {
                  e.preventDefault();
                  const next = e.key === "Home" ? from : e.key === "End" ? to : addDays(d, steps[e.key]!);
                  const target = next < from ? from : next > to ? to : next;
                  setFocusDate(target);
                  calendarRef.current?.querySelector<SVGRectElement>(`[data-day="${target}"]`)?.focus();
                } else if (p && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); onPick(d); }
              }}
              style={p ? undefined : { cursor: "default" }}
            >
              <title>{label}</title>
            </rect>
          );
        }
        const label = monthYear(first);
        return (
          <div key={m} className="heat-month">
            <h3>{label}</h3>
            <div className="month-body"><div className="heat-wd" aria-hidden="true">{[0, 1, 2, 3, 4, 5, 6].map((r) => <span key={r} style={{ height: step }}>{WD_LABEL[r] ?? ""}</span>)}</div><svg width={W} height={H} role="group" aria-label={`${label}: ${recorded} nights recorded, ${valid} with more than 4 hours.`}>{cells}</svg></div>
          </div>
        );
      })}
    </div>
  );
}
