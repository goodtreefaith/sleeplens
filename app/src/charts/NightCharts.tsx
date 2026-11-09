import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import uPlot from "uplot";
import type { NightDetail, PressureUnit, TherapyEvent } from "../api/types";
import { Hypnogram, type HypnogramScale } from "../components/SleepStages";
import { HelpTip } from "../components/HelpTip";
import { EVENT_STYLE, Icon, type MarkerKind, shapePath, Swatch } from "../components/Icon";
import { wallSeconds } from "../lib/dates";
import { r1, unitLabel } from "../lib/format";
import { cssVar } from "../lib/theme";

export interface Zoom { min: number; max: number }

export { stageRuns } from "../lib/stages";

interface Props {
  detail: NightDetail;
  unit: PressureUnit;
  theme: "light" | "dark";
  zoom: Zoom | null;
  onZoom: (z: Zoom | null) => void;
}

/** Uses the same near-minimum tolerance as pressure insights, including ramp. */
export function eventAtMinimum(event: TherapyEvent, minimum: number | null | undefined): boolean {
  return minimum != null && event.pressure != null && (event.kind === "obstructiveApnea" || event.kind === "hypopnea") && Math.abs(event.pressure - minimum) <= 0.5;
}

export function pressureSettingBounds(detail: Pick<NightDetail, "settings">): number[] {
  return detail.settings ? [...new Set([detail.settings.minPressure, detail.settings.maxPressure])] : [];
}

/** Open downward caret, distinct from every breathing-event marker. */
export function minimumPressureMarkerPath(cx: number, cy: number, r: number): string {
  return `M${cx - r} ${cy - r / 2}L${cx} ${cy + r / 2}L${cx + r} ${cy - r / 2}`;
}

export const NIGHT_CHART_AXIS_WIDTH = 84;
export const NIGHT_CHART_PADDING: [number, number, number, number] = [8, 12, 0, 0];

export function nightHypnogramScale(plot: Pick<uPlot, "bbox" | "scales">, pixelRatio: number): HypnogramScale {
  return { min: plot.scales.x!.min!, max: plot.scales.x!.max!, left: plot.bbox.left / pixelRatio, right: (plot.bbox.left + plot.bbox.width) / pixelRatio };
}
const MARKER_KINDS: MarkerKind[] = ["obstructiveApnea", "centralApnea", "hypopnea", "largeLeak"];

/** Resolves var(--x) to a colour value uPlot (canvas) can use. */
function col(v: string): string {
  const m = /^var\((--[\w-]+)\)$/.exec(v);
  return m ? cssVar(m[1]!) : v;
}

const clockFmt = (s: number) => new Date(s * 1000).toISOString().slice(11, 16);

export function NightCharts({ detail, unit, theme, zoom, onZoom }: Props) {
  const series = detail.series!;
  const t0 = wallSeconds(series.seriesStart);
  const n = series.pressure.length;
  const x = useMemo(() => {
    const a = new Array<number>(n);
    for (let i = 0; i < n; i++) a[i] = t0 + i;
    return a;
  }, [t0, n]);

  const markers = useMemo(
    () => detail.events
      .filter((e) => (MARKER_KINDS as string[]).includes(e.kind))
      .map((e) => ({ t: wallSeconds(e.time), kind: e.kind as MarkerKind, lane: MARKER_KINDS.indexOf(e.kind as MarkerKind) })),
    [detail.events]
  );
  const breathing = useMemo(() => markers.filter((m) => m.kind !== "largeLeak").map((m) => m.t), [markers]);

  const refs = [useRef<HTMLDivElement>(null), useRef<HTMLDivElement>(null), useRef<HTMLDivElement>(null), useRef<HTMLDivElement>(null)];
  const plots = useRef<uPlot[]>([]);
  const syncing = useRef(false);
  const onZoomRef = useRef(onZoom);
  onZoomRef.current = onZoom;
  const [readout, setReadout] = useState<string>("");
  const [stageScale, setStageScale] = useState<HypnogramScale>();
  const u = unitLabel(unit);

  useEffect(() => {
    const els = refs.map((r) => r.current);
    if (els.some((e) => !e)) return;
    const muted = cssVar("--muted");
    const grid = cssVar("--line");
    const font = `12px ${cssVar("--font") || "system-ui"}`;
    const tzDate = (ts: number) => uPlot.tzDate(new Date(ts * 1e3), "Etc/UTC");
    const width = els[0]!.clientWidth || 800;
    const syncKey = "night-" + detail.date;

    const eventLines: uPlot.Plugin = {
      hooks: {
        drawClear: (p: uPlot) => {
          const ctx = p.ctx;
          ctx.save();
          ctx.strokeStyle = cssVar("--ev1");
          ctx.globalAlpha = 0.22;
          ctx.lineWidth = Math.max(1, devicePixelRatio);
          const { left, top, height } = p.bbox;
          for (const t of breathing) {
            if (t < (p.scales.x!.min ?? 0) || t > (p.scales.x!.max ?? 0)) continue;
            const px = Math.round(p.valToPos(t, "x", true));
            ctx.beginPath(); ctx.moveTo(px, top); ctx.lineTo(px, top + height); ctx.stroke();
          }
          ctx.restore();
          void left;
        }
      }
    };

    const wheelZoom: uPlot.Plugin = {
      hooks: {
        ready: (p: uPlot) => {
          p.over.addEventListener("wheel", (e: WheelEvent) => {
            if (!e.ctrlKey && !e.metaKey) return; // trackpad pinch, or Cmd + scroll; plain scroll keeps scrolling the page
            e.preventDefault();
            const xMin = p.scales.x!.min!, xMax = p.scales.x!.max!;
            const at = p.posToVal(e.offsetX, "x");
            const f = e.deltaY < 0 ? 0.8 : 1.25;
            let nMin = at - (at - xMin) * f, nMax = at + (xMax - at) * f;
            const full0 = x[0]!, full1 = x[x.length - 1]!;
            if (nMax - nMin < 120) return;
            nMin = Math.max(full0, nMin); nMax = Math.min(full1, nMax);
            p.setScale("x", { min: nMin, max: nMax });
          }, { passive: false });
        }
      }
    };

    const propagate = (p: uPlot, key: string) => {
      if (key !== "x" || syncing.current) return;
      const { min, max } = p.scales.x!;
      if (min == null || max == null) return;
      syncing.current = true;
      plots.current.forEach((o) => { if (o !== p) o.setScale("x", { min, max }); });
      syncing.current = false;
      const full = min <= x[0]! + 1 && max >= x[x.length - 1]! - 1;
      onZoomRef.current(full ? null : { min, max });
    };

    const syncStageScale = (p: uPlot) => {
      const next = nightHypnogramScale(p, devicePixelRatio);
      setStageScale(old => old && Object.keys(next).every(key => old[key as keyof HypnogramScale] === next[key as keyof HypnogramScale]) ? old : next);
    };

    const base = (height: number, showX: boolean): Omit<uPlot.Options, "series"> => ({
      width,
      height,
      tzDate,
      padding: NIGHT_CHART_PADDING,
      legend: { show: false },
      cursor: { sync: { key: syncKey, setSeries: false }, drag: { x: true, y: false, setScale: true }, points: { size: 6 } },
      scales: { x: { time: true, min: x[0], max: x[x.length - 1] } },
      axes: [
        showX
          ? { stroke: muted, grid: { stroke: grid, width: 1 }, ticks: { stroke: grid }, font, values: (_p, splits) => splits.map(clockFmt) }
          : { stroke: muted, grid: { stroke: grid, width: 1 }, ticks: { show: false }, size: 4, values: (_p, splits) => splits.map(() => "") },
        { stroke: muted, grid: { stroke: grid, width: 1 }, ticks: { show: false }, font, size: NIGHT_CHART_AXIS_WIDTH }
      ],
      hooks: { setScale: [propagate] },
      plugins: [eventLines, wheelZoom]
    });

    const settingLines: uPlot.Plugin = {
      hooks: { draw: (p: uPlot) => {
        const ctx = p.ctx;
        const ratio = devicePixelRatio;
        const { left, top, width: chartWidth, height } = p.bbox;
        ctx.save();
        ctx.beginPath(); ctx.rect(left, top, chartWidth, height); ctx.clip();
        ctx.strokeStyle = muted;
        ctx.lineWidth = ratio;
        ctx.setLineDash([4 * ratio, 4 * ratio]);
        for (const setting of pressureSettingBounds(detail)) {
          const y = p.valToPos(setting, "y", true);
          ctx.beginPath(); ctx.moveTo(left, y); ctx.lineTo(left + chartWidth, y); ctx.stroke();
        }
        ctx.setLineDash([]);
        for (const event of detail.events.filter(event => eventAtMinimum(event, detail.settings?.minPressure))) {
          const t = wallSeconds(event.time);
          if (t < p.scales.x!.min! || t > p.scales.x!.max!) continue;
          const px = p.valToPos(t, "x", true);
          const py = p.valToPos(event.pressure!, "y", true);
          ctx.strokeStyle = cssVar("--ev1");
          ctx.lineWidth = 2 * ratio;
          const path = new Path2D(minimumPressureMarkerPath(px, py, 5 * ratio));
          ctx.stroke(path);
        }
        ctx.restore();
      } }
    };

    const pressureOpts: uPlot.Options = {
      ...base(150, false),
      scales: { x: { time: true, min: x[0], max: x[x.length - 1] }, y: { range: (_p, mn, mx) => [Math.floor(Math.min(mn ?? 0, ...pressureSettingBounds(detail)) - 0.5), Math.ceil(Math.max(mx ?? 10, ...pressureSettingBounds(detail)) + 0.5)] } },
      plugins: [eventLines, wheelZoom, settingLines],
      series: [{}, { label: `Pressure (${u})`, stroke: col("var(--p-line)"), width: 1.5, points: { show: false } }],
      hooks: {
        setScale: [propagate],
        setCursor: [(p: uPlot) => {
          const i = p.cursor.idx;
          if (i == null) { setReadout(""); return; }
          const parts = [clockFmt(x[i]!)];
          const pv = series.pressure[i], lv = series.excessLeak[i], fv = series.flowAmplitude[i];
          if (pv == null) parts.push("mask off");
          else parts.push(`pressure ${r1(pv)} ${u}`, `extra leak ${r1(lv)} L/min`, `flow ${r1(fv)} L/min`);
          setReadout(parts.join(" · "));
        }]
      }
    };
    const leakOpts: uPlot.Options = {
      ...base(130, false),
      scales: { x: { time: true, min: x[0], max: x[x.length - 1] }, y: { range: (_p, _mn, mx) => [0, Math.max(10, Math.ceil(((mx ?? 0) + 2) / 5) * 5)] } },
      series: [{}, { label: "Extra leak (L/min)", stroke: col("var(--leak-line)"), fill: col("var(--leak-fill)"), width: 1, points: { show: false } }]
    };
    const flowOpts: uPlot.Options = {
      ...base(130, false),
      scales: { x: { time: true, min: x[0], max: x[x.length - 1] }, y: { range: (_p, _mn, mx) => [0, Math.ceil(((mx ?? 30) + 2) / 10) * 10] } },
      series: [{}, { label: "Flow amplitude (L/min)", stroke: col("var(--amp-line)"), width: 1, points: { show: false } }]
    };
    const laneOpts: uPlot.Options = {
      ...base(150, true),
      scales: { x: { time: true, min: x[0], max: x[x.length - 1] }, y: { range: [-0.5, 3.5] } },
      axes: [
        { stroke: muted, grid: { stroke: grid, width: 1 }, ticks: { stroke: grid }, font, values: (_p, splits) => splits.map(clockFmt) },
        { stroke: muted, grid: { show: false }, ticks: { show: false }, font, size: NIGHT_CHART_AXIS_WIDTH, splits: () => [3, 2, 1, 0], values: () => ["Obstructive", "Central", "Hypopnea", "Large leak"] }
      ],
      series: [{}, { show: false }],
      plugins: [wheelZoom, {
        hooks: {
          ready: syncStageScale,
          setSize: syncStageScale,
          setScale: syncStageScale,
          draw: (p: uPlot) => {
            const ctx = p.ctx;
            const r = 5 * devicePixelRatio;
            ctx.save();
            ctx.lineWidth = 1.5 * devicePixelRatio;
            ctx.strokeStyle = cssVar("--surface");
            for (const m of markers) {
              if (m.t < p.scales.x!.min! || m.t > p.scales.x!.max!) continue;
              const cx = p.valToPos(m.t, "x", true);
              const cy = p.valToPos(3 - m.lane, "y", true);
              const s = EVENT_STYLE[m.kind];
              ctx.fillStyle = col(s.color);
              const path = new Path2D(shapePath(s.shape, cx, cy, r));
              ctx.fill(path); ctx.stroke(path);
            }
            ctx.restore();
          }
        }
      }]
    };

    const make = (opts: uPlot.Options, ys: (number | null)[], el: HTMLDivElement) => new uPlot(opts, [x, ys] as uPlot.AlignedData, el);
    const nulls = new Array<number | null>(n).fill(null);
    plots.current = [
      make(pressureOpts, series.pressure, els[0]!),
      make(leakOpts, series.excessLeak, els[1]!),
      make(flowOpts, series.flowAmplitude, els[2]!),
      make(laneOpts, nulls, els[3]!)
    ];

    const ro = new ResizeObserver(() => {
      const w = els[0]!.clientWidth;
      plots.current.forEach((p) => { if (Math.abs(p.width - w) > 1) p.setSize({ width: w, height: p.height }); });
    });
    ro.observe(els[0]!);
    return () => {
      ro.disconnect();
      plots.current.forEach((p) => p.destroy());
      plots.current = [];
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail, theme, unit, x, markers, breathing]);

  // External zoom (event list, buttons).
  useEffect(() => {
    const p = plots.current[0];
    if (!p) return;
    const min = zoom?.min ?? x[0]!;
    const max = zoom?.max ?? x[x.length - 1]!;
    if (Math.abs((p.scales.x!.min ?? 0) - min) < 1 && Math.abs((p.scales.x!.max ?? 0) - max) < 1) return;
    p.setScale("x", { min, max });
  }, [zoom, x]);

  const rows: { label: string; sub: string; ref?: RefObject<HTMLDivElement>; help?: string }[] = [
    { label: "Pressure", sub: u, help: "pressure", ref: refs[0]! },
    { label: "Extra leak", sub: "L/min, beyond the vent", help: "excessLeak", ref: refs[1]! },
    { label: "Flow amplitude", sub: "L/min, size of each breath", ref: refs[2]! },
    { label: "Sleep stages", sub: detail.stages ? "Estimated from breathing and not validated for your CPAP device. Uncertain means the signal cannot support a stage claim; deep sleep is the least reliable estimate." : "Not recorded. Re-read the card to add stages.", help: "sleepStages" },
    { label: "Events", sub: "each mark is one event", ref: refs[3]! }
  ];

  return (
    <div>
      <div className="chart-tools unified-chart-tools">
        <div className="legend" aria-label="Events and estimated sleep stages">
          {MARKER_KINDS.map((k) => <span key={k}><Swatch kind={k} />{EVENT_STYLE[k].short} <HelpTip term={k} /></span>)}
          <span><svg viewBox="0 0 14 14" aria-hidden="true"><path d="M7 1v12" stroke="var(--ev1)" strokeOpacity=".5" strokeWidth="2" /></svg>Event time</span>

        </div>
        <button className="btn" type="button" aria-disabled={!zoom} title="Drag across a chart to zoom in. Pinch or hold Cmd and scroll to zoom around the pointer. Select Whole night to reset." onClick={() => zoom && onZoom(null)}><Icon name="zoomOut" />Whole night</button>
      </div>
      {detail.settings ? <div className="pressure-chart-legend" aria-label="Pressure chart annotations">
        <span><svg viewBox="0 0 18 14" aria-hidden="true"><path d="M0 7h18" stroke="currentColor" strokeDasharray="4 3" /></svg>Min {detail.settings.minPressure} / Max {detail.settings.maxPressure} {u} <HelpTip term="pressureRange" /></span>
        <span><svg viewBox="0 0 18 14" aria-hidden="true"><path d={minimumPressureMarkerPath(9, 7, 6)} fill="none" stroke="var(--ev1)" strokeWidth="2" /></svg>Obstructive / hypopnea at minimum ({detail.events.filter(event => eventAtMinimum(event, detail.settings?.minPressure)).length}) <HelpTip term="pressureEventsAtMin" /></span>
      </div> : null}
      <p className="readout" aria-live="off">{readout || "\u00a0"}</p>
      <div className="charts">
        {rows.map((r) => (
          <div className="chart-row" key={r.label}>
            <div className="lbl"><b>{r.label} {r.help ? <HelpTip term={r.help} /> : null}</b><span>{r.sub}</span></div>
            {r.label === "Sleep stages" ? <div className="plot stage-lane"><Hypnogram stages={detail.stages} from={zoom?.min ?? x[0]} to={zoom?.max ?? x[x.length - 1]} axisWidth={NIGHT_CHART_AXIS_WIDTH} chartScale={stageScale} /></div> : <div className="plot" ref={r.ref} role="img" aria-label={`${r.label} through the night`} />}
          </div>
        ))}
      </div>
    </div>
  );
}
