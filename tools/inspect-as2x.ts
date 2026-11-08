/**
 * Usage: tsx tools/inspect-as2x.ts <cardDir> [--json] [--night YYYYMMDD]
 * Aeonmed AS100 series cards (internally "AS2X"). Read-only. The card serial is
 * always shown as <SERIAL>.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { detectAs2x } from '../src/parsers/as2x/detect';
import { summarizeDat } from '../src/parsers/as2x/dat';
import { parseBkp } from '../src/parsers/as2x/bkp';
import type { BkpParseResult, DatSummary, SessionSummary, TherapyEvent } from '../src/parsers/as2x/types';

const args = process.argv.slice(2);
const json = args.includes('--json');
const nightIdx = args.indexOf('--night');
const onlyNight = nightIdx >= 0 ? args[nightIdx + 1] : undefined;
const cardDir = args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--night');

if (!cardDir || (nightIdx >= 0 && !/^\d{8}$/.test(onlyNight ?? ''))) {
  console.error('Usage: tsx tools/inspect-as2x.ts <cardDir> [--json] [--night YYYYMMDD]');
  process.exit(1);
}

/** Relative file paths, two levels deep (month folders + root files). Names only, no reads. */
function listCard(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    if (name.startsWith('.')) continue;
    const p = join(dir, name);
    let st;
    try {
      st = statSync(p);
    } catch {
      continue;
    }
    if (st.isDirectory()) {
      for (const f of readdirSync(p)) if (!f.startsWith('.')) out.push(`${name}/${f}`);
    } else out.push(name);
  }
  return out;
}

const t0 = Date.now();
const detection = detectAs2x(listCard(cardDir));
if (!detection.detected || !detection.serial) {
  console.error('No Aeonmed AS100 series card layout found (expected <serial><YYYYMM>/<YYYYMMDD>.dat).');
  process.exit(2);
}
const serial = detection.serial;
const redact = (s: string) => s.split(serial).join('<SERIAL>');

let bkp: BkpParseResult | null = null;
if (detection.bkp) bkp = parseBkp(readFileSync(join(cardDir, detection.bkp)));

const nights = onlyNight ? detection.nights.filter((n) => n.date === onlyNight) : detection.nights;
if (nights.length === 0) {
  console.error(`No night file ${onlyNight}.`);
  process.exit(3);
}

interface NightRow {
  date: string;
  file: string;
  start: string | null;
  end: string | null;
  usageMinutes: number;
  pressureMean: number;
  pressureP95: number;
  leakMedian: number;
  session: SessionSummary | null;
  eventCounts: Record<string, number> | null;
  recordCount: number;
  invalidRecords: number;
}

function match(sum: DatSummary): { session: SessionSummary | null; events: TherapyEvent[] } {
  if (!bkp || !sum.start || !sum.end) return { session: null, events: [] };
  const lo = sum.start.epochSec - 120;
  const hi = sum.end.epochSec + 120;
  const session = bkp.sessions.find((s) => s.start && s.start.epochSec >= lo && s.start.epochSec <= hi) ?? null;
  const events = session ? bkp.events.filter((e) => e.time.epochSec >= lo && e.time.epochSec <= hi) : [];
  return { session, events };
}

const rows: NightRow[] = [];
const detail: Array<{ events: TherapyEvent[] }> = [];
for (const n of nights) {
  const buf = readFileSync(join(cardDir, n.path)); // one file at a time
  const sum = summarizeDat(buf, { includeSeries: false });
  const { session, events } = match(sum);
  let counts: Record<string, number> | null = null;
  if (session) {
    counts = {};
    for (const e of events) counts[e.eventType] = (counts[e.eventType] ?? 0) + 1;
  }
  rows.push({
    date: n.date,
    file: redact(n.path),
    start: sum.start?.iso ?? null,
    end: sum.end?.iso ?? null,
    usageMinutes: Math.round(sum.usageMinutes * 10) / 10,
    pressureMean: Math.round(sum.pressure.mean * 10) / 10,
    pressureP95: sum.pressure.p95,
    leakMedian: sum.leak.median,
    session,
    eventCounts: counts,
    recordCount: sum.recordCount,
    invalidRecords: sum.invalidRecords,
  });
  detail.push({ events });
}

const elapsed = ((Date.now() - t0) / 1000).toFixed(1);

if (json) {
  const events = onlyNight && detail[0] ? detail[0].events.map(({ time, ...e }) => ({ time: time.iso, ...e })) : undefined;
  console.log(JSON.stringify({ serial: '<SERIAL>', nights: rows, ...(events ? { events } : {}) }, null, 2));
} else {
  const hm = (min: number) => {
    const t = Math.round(min);
    return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
  };
  const clock = (iso: string | null) => (iso ? iso.slice(11, 16) : '--:--');
  console.log(
    `Aeonmed AS100 series card <SERIAL>: ${detection.nights.length} nights, ${detection.months.length} months, bkp ${bkp ? 'found' : 'missing'}`,
  );
  console.log('date      start-end    usage  mean  p95  leak   AHI  events');
  for (const r of rows) {
    const ev = r.eventCounts
      ? Object.entries(r.eventCounts)
          .filter(([k]) => ['apnea1', 'apnea2', 'hypopnea'].includes(k))
          .map(([k, v]) => `${v} ${k}`)
          .join(' + ') || '0'
      : '-';
    console.log(
      [
        r.date,
        `${clock(r.start)}-${clock(r.end)}`,
        hm(r.usageMinutes).padStart(5),
        r.pressureMean.toFixed(1).padStart(5),
        r.pressureP95.toFixed(1).padStart(4),
        String(r.leakMedian).padStart(4),
        (r.session ? r.session.ahi.toFixed(1) : '-').padStart(5),
        ev,
      ].join('  '),
    );
  }
  if (onlyNight && rows[0]) {
    const r = rows[0];
    console.log(`\nNight ${r.date}: ${r.recordCount} records (${r.invalidRecords} invalid), ${r.usageMinutes} min`);
    if (r.session) {
      const s = r.session;
      console.log(
        `bkp summary: session ${s.sessionNumber}, usage ${s.usageMinutes} min, pressure min/mean/p95 ${s.pressure.min}/${s.pressure.mean}/${s.pressure.p95}, AHI ${s.ahi}, AI ${s.apneaIndex}, HI ${s.hypopneaIndex}`,
      );
      for (const e of detail[0]!.events)
        console.log(`  ${e.time.iso.slice(11)}  ${e.eventType} ${e.pressure ?? e.value}`);
    } else console.log('No matching bkp session summary (older than the ring buffer, or no bkp).');
  }
  console.error(`(${elapsed}s)`);
}
