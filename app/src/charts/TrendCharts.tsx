import { useState } from "react";
import type { TrendPoint } from "../api/types";
import { addDays, dateMs, daysInclusive, DAY_MS } from "../lib/dates";
import { hm, monthYear, shortDate, shortDateY } from "../lib/format";
import { useWidth } from "../lib/useWidth";

/** Small line chart with a guide line and prescription-change markers. */
export function LineChart({ points, from, to, value, guide, guideLabel, unit, aria, onPick, changeDescriptions = {}, max }: {
  points: TrendPoint[]; from: string; to: string;
  value: (p: TrendPoint) => number | null;
  guide?: number; guideLabel?: string; changeDescriptions?: Record<string, string>; max?: number; unit: string; aria: string;
  onPick: (d: string) => void;
}) {
  const [ref, w] = useWidth<HTMLDivElement>();
  const [activeMarker, setActiveMarker] = useState<string | null>(null);
  const h = 170, padL = 36, padR = 8, padT = 24, padB = 24;
  const vals = points.map(value).filter((v): v is number => v != null);
  const maxV = max ?? Math.max(guide ?? 0, ...vals, 1) * 1.15;
  const minV = 0;
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
    if (v != null) cur += `${cur ? "L" : "M"}${sx(p.date).toFixed(1)} ${sy(v).toFixed(1)}`;
    prevDate = p.date;
  }
  if (cur) segs.push(cur);
  const ticksY = niceTicks(maxV);
  const xt = dateTicks(from, to, w);
  const changes = points.filter((p) => p.prescriptionChanged);
  const dots = points.length <= 40;
  return (
    <div className="chartwrap" ref={ref}>
      {guideLabel ? <p className="chart-guide-label">General guide: {guideLabel}</p> : null}
      {activeMarker ? <p className="chart-marker-tooltip" role="status">{activeMarker}</p> : null}
      <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} role="img" aria-label={aria}>
        {ticksY.map((t) => (
          <g key={t}>
            <line className="gridline" x1={padL} x2={w - padR} y1={sy(t)} y2={sy(t)} />
            <text className="tick" x={padL - 6} y={sy(t) + 4} textAnchor="end">{t}</text>
          </g>
        ))}
        {guide != null ? (
          <g>
            <line className="guide" x1={padL} x2={w - padR} y1={sy(guide)} y2={sy(guide)} />
          </g>
        ) : null}
        {segs.map((d, i) => <path key={i} className="lineplot" d={d} />)}
        {dots ? points.map((p) => { const v = value(p); return v == null ? null : <circle key={p.date} cx={sx(p.date)} cy={sy(v)} r={2.5} fill="var(--accent)" />; }) : null}
        {xt.map((t) => <text key={t} className="tick" x={sx(t)} y={h - 6} textAnchor="middle">{shortDate(t)}</text>)}
        {points.map((p) => {
          const v = value(p);
          return (
            <rect key={"h" + p.date} x={sx(p.date) - 4} y={padT} width={8} height={h - padT - padB} fill="transparent" style={{ cursor: "pointer" }} onClick={() => onPick(p.date)} tabIndex={0} role="button" aria-label={`${shortDateY(p.date)}, ${v == null ? "not recorded" : `${v.toFixed(1)} ${unit}`}. Open night`} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onPick(p.date); } }}>
              <title>{`${shortDateY(p.date)}: ${v == null ? "not recorded" : `${v.toFixed(1)} ${unit}`}`}</title>
            </rect>
          );
        })}
        {changes.map((c) => {
          const text = `${shortDateY(c.date)}: ${changeDescriptions[c.date] || "Mode or pressure prescription changed"}`;
          return <g key={"change" + c.date} className="prescription-marker" tabIndex={0} role="img" aria-label={text} onFocus={() => setActiveMarker(text)} onBlur={() => setActiveMarker(null)} onMouseEnter={() => setActiveMarker(text)} onMouseLeave={() => setActiveMarker(null)} onKeyDown={(e) => { if (e.key === "Escape") setActiveMarker(null); }}>
            <title>{text}</title>
            <line className="mk" x1={sx(c.date)} x2={sx(c.date)} y1={padT - 6} y2={h - padB} />
            <line x1={sx(c.date)} x2={sx(c.date)} y1={padT - 6} y2={h - padB} stroke="transparent" strokeWidth={10} />
          </g>;
        })}
      </svg>
    </div>
  );
}

function niceTicks(max: number): number[] {
  const step = max <= 3 ? 1 : max <= 8 ? 2 : max <= 20 ? 5 : max <= 50 ? 10 : 20;
  const out: number[] = [];
  for (let v = 0; v <= max; v += step) out.push(v);
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
              tabIndex={p ? 0 : undefined}
              role={p ? "button" : undefined}
              aria-label={p ? `${label}. Open this night` : undefined}
              onClick={p ? () => onPick(d) : undefined}
              onKeyDown={p ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onPick(d); } } : undefined}
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
