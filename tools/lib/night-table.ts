/**
 * Night rows and the night table for tools/inspect-as2x.ts. Pure functions, so
 * tests can check the summary-versus-.dat choice and the text layout.
 */
import type { DatSummary, PressureUnitLabel, SessionSummary } from '../../src/parsers/as2x/types';

/** Where a night's usage, mean and P90 came from. */
export type ValueSource = 'summary' | 'dat';

export interface NightRow {
  date: string;
  file: string;
  start: string | null;
  end: string | null;
  /** bkp summary 0x54 when there is a summary for the night, else the .dat active time (see `usageSource`). */
  usageMinutes: number;
  usageSource: ValueSource;
  /** bkp summary 0x4E when there is a summary, else the .dat per-second mean (see `pressureSource`). */
  pressureMean: number;
  /** bkp summary 0x50 when there is a summary, else the .dat nearest-rank P90 (see `pressureSource`). */
  pressureP90: number;
  pressureSource: ValueSource;
  /** The .dat values, always present. The machine screen matches the summary, not these. */
  datUsageMinutes: number;
  datPressureMean: number;
  datPressureP90: number;
  activeIntervals: number;
  /** @deprecated same as `totalLeakMedian` (slot 384). */
  leakMedian: number;
  /** @deprecated same as `excessLeakMedian` (slots 270-319). */
  leakFlowMedian: number;
  /** Median excess (unintentional) leak, slots 270-319, L/min (Likely). */
  excessLeakMedian: number;
  /** Median total leak including vent flow, slot 384, L/min (Likely). */
  totalLeakMedian: number;
  /** bkp summary 0x52, minutes with leak above 90 L/min (Likely); null without a summary. */
  largeLeakMinutes: number | null;
  session: SessionSummary | null;
  eventCounts: Record<string, number> | null;
  sessions: Array<{ start: string; end: string; minutes: number }>;
  recordCount: number;
  invalidRecords: number;
}

const round1 = (x: number) => Math.round(x * 10) / 10;

export function buildNightRow(
  night: { date: string; file: string },
  sum: DatSummary,
  session: SessionSummary | null,
  eventCounts: Record<string, number> | null,
): NightRow {
  const datUsageMinutes = round1(sum.usageMinutes);
  const datPressureMean = round1(sum.pressure.mean);
  const datPressureP90 = sum.pressure.p90;
  return {
    date: night.date,
    file: night.file,
    start: sum.start?.iso ?? null,
    end: sum.end?.iso ?? null,
    usageMinutes: session ? session.usageMinutes : datUsageMinutes,
    usageSource: session ? 'summary' : 'dat',
    pressureMean: session ? session.pressure.mean : datPressureMean,
    pressureP90: session ? session.pressure.p90 : datPressureP90,
    pressureSource: session ? 'summary' : 'dat',
    datUsageMinutes,
    datPressureMean,
    datPressureP90,
    activeIntervals: sum.sessions.length,
    leakMedian: sum.totalLeak.median,
    leakFlowMedian: sum.excessLeak.median,
    excessLeakMedian: sum.excessLeak.median,
    totalLeakMedian: sum.totalLeak.median,
    largeLeakMinutes: session ? session.largeLeakMinutes : null,
    session,
    eventCounts,
    sessions: sum.sessions.map((x) => ({ start: x.start.iso, end: x.end.iso, minutes: round1(x.minutes) })),
    recordCount: sum.recordCount,
    invalidRecords: sum.invalidRecords,
  };
}

/** Display name for card pressures. Unknown unit bytes are not guessed. */
export function pressureUnitText(label: PressureUnitLabel | null | undefined): string {
  return label === 'hPa' ? 'hPa' : 'card units';
}

const SHORT: Record<string, string> = { obstructiveApnea: 'OA', centralApnea: 'CA', hypopnea: 'H' };

const hm = (min: number) => {
  const t = Math.round(min);
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
};
const clock = (iso: string | null) => (iso ? iso.slice(11, 16) : '--:--');
/** Value plus a one-character marker: '*' for a .dat fallback, else a space. */
const mark = (text: string, source: ValueSource, width: number) => `${text}${source === 'dat' ? '*' : ' '}`.padStart(width + 1);

/** The night table: a header, one line per night, then the units and footnote lines. */
export function formatNightTable(rows: NightRow[], unit: PressureUnitLabel | null | undefined): string[] {
  const out: string[] = [];
  out.push(
    `${'date'.padEnd(8)}  ${'start-end'.padEnd(11)}  ${mark('usage', 'summary', 5)}  ${mark('mean', 'summary', 4)}  ${mark('p90', 'summary', 4)}  ${'excess leak'.padStart(11)}  ${'large leak min'.padStart(14)}  ${'AHI'.padStart(5)}  events (OA obstructive, CA central, H hypopnea)`,
  );
  for (const r of rows) {
    const ev = r.eventCounts
      ? Object.entries(r.eventCounts)
          .filter(([k]) => k in SHORT)
          .map(([k, v]) => `${v} ${SHORT[k]}`)
          .join(' + ') || '0'
      : '-';
    out.push(
      [
        r.date,
        `${clock(r.start)}-${clock(r.end)}`,
        mark(hm(r.usageMinutes), r.usageSource, 5),
        mark(r.pressureMean.toFixed(1), r.pressureSource, 4),
        mark(r.pressureP90.toFixed(1), r.pressureSource, 4),
        r.excessLeakMedian.toFixed(1).padStart(11),
        (r.largeLeakMinutes === null ? '-' : r.largeLeakMinutes.toFixed(1)).padStart(14),
        (r.session ? r.session.ahi.toFixed(1) : '-').padStart(5),
        ev,
      ].join('  '),
    );
  }
  const unitText = pressureUnitText(unit);
  out.push(
    `pressure in ${unitText}${unit === 'hPa' ? ' (about the same number in cmH2O)' : ' (pressure unit byte not recognised)'}; usage, mean and p90 from the bkp session summary when there is one; excess leak = median L/min of slots 270-319; large leak min = minutes above 90 L/min`,
  );
  if (rows.some((r) => r.usageSource === 'dat' || r.pressureSource === 'dat'))
    out.push('* no bkp session summary for this night: value computed from the night file');
  return out;
}
