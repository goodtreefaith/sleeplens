/**
 * Usage: tsx tools/inspect-as2x.ts <cardDir> [--json] [--night YYYYMMDD]
 * Aeonmed AS100 series cards (internally "AS2X"). Read-only. The card serial is
 * always shown as <SERIAL>.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { detectAs2x } from '../src/parsers/as2x/detect';
import { parseDatHeader, summarizeDat } from '../src/parsers/as2x/dat';
import { parseBkp } from '../src/parsers/as2x/bkp';
import type { BkpParseResult, DatHeader, DatSummary, SessionSummary, TherapyEvent } from '../src/parsers/as2x/types';

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
  pressureP90: number;
  activeIntervals: number;
  leakMedian: number;
  /** Median of NeoRead's leak flow channel (slots 270-319), L/min. */
  leakFlowMedian: number;
  session: SessionSummary | null;
  eventCounts: Record<string, number> | null;
  sessions: Array<{ start: string; end: string; minutes: number }>;
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
let header: DatHeader | null = null;
for (const n of nights) {
  const buf = readFileSync(join(cardDir, n.path)); // one file at a time
  if (!header) header = parseDatHeader(buf);
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
    pressureP90: sum.pressure.p90,
    activeIntervals: sum.sessions.length,
    leakMedian: sum.leak.median,
    leakFlowMedian: sum.leakFlow.median,
    session,
    eventCounts: counts,
    recordCount: sum.recordCount,
    sessions: sum.sessions.map((x) => ({ start: x.start.iso, end: x.end.iso, minutes: Math.round(x.minutes * 10) / 10 })),
    invalidRecords: sum.invalidRecords,
  });
  detail.push({ events });
}

const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
// Device facts from the first night's header. The device ID itself carries the serial, so it is never printed.
const device = header
  ? {
      model: header.modelName ?? 'unknown',
      idPrefix: header.deviceModel,
      formatVersion: header.formatVersion,
      softwareVersion: header.softwareVersion,
      hardwareVersion: header.hardwareVersion,
    }
  : null;
const settings = header?.settings ?? null;

if (json) {
  const events = onlyNight && detail[0] ? detail[0].events.map(({ time, ...e }) => ({ time: time.iso, ...e })) : undefined;
  const settingsOut = settings ? (({ raw: _raw, time, ...rest }) => ({ time: time?.iso ?? null, ...rest }))(settings) : null;
  console.log(
    JSON.stringify({ serial: '<SERIAL>', device, settings: settingsOut, nights: rows, ...(events ? { events } : {}) }, null, 2),
  );
} else {
  const hm = (min: number) => {
    const t = Math.round(min);
    return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
  };
  const clock = (iso: string | null) => (iso ? iso.slice(11, 16) : '--:--');
  console.log(
    `Aeonmed AS100 series card <SERIAL>: ${detection.nights.length} nights, ${detection.months.length} months, bkp ${bkp ? 'found' : 'missing'}`,
  );
  if (device)
    console.log(
      `device: ${device.model} (${device.idPrefix}), firmware ${device.softwareVersion ?? '?'}, hardware ${device.hardwareVersion ?? '?'}, night file format v${device.formatVersion}`,
    );
  if (settings)
    console.log(
      `settings (first night): mode ${settings.modeLabel}, start ${settings.startPressure}, min ${settings.minPressure}, max ${settings.maxPressure}, fixed ${settings.fixedPressure} cmH2O, ramp ${settings.rampMinutes} min, humidifier ${settings.humidifierLevel}, AS-Elex ${settings.comfortLevel}, auto on ${settings.autoOn ? 'yes' : 'no'}, auto off ${settings.autoOff ? 'yes' : 'no'}`,
    );
  if (header && !header.recordsSupported) console.log('Night files use the version 1 (512-byte) record layout, which is not decoded.');
  const SHORT: Record<string, string> = { obstructiveApnea: 'OA', centralApnea: 'CA', hypopnea: 'H' };
  console.log('date      start-end    usage  mean   p90  leak   AHI  events (OA obstructive, CA central, H hypopnea)');
  for (const r of rows) {
    const ev = r.eventCounts
      ? Object.entries(r.eventCounts)
          .filter(([k]) => k in SHORT)
          .map(([k, v]) => `${v} ${SHORT[k]}`)
          .join(' + ') || '0'
      : '-';
    console.log(
      [
        r.date,
        `${clock(r.start)}-${clock(r.end)}`,
        hm(r.usageMinutes).padStart(5),
        r.pressureMean.toFixed(1).padStart(5),
        r.pressureP90.toFixed(1).padStart(5),
        String(r.leakMedian).padStart(4),
        (r.session ? r.session.ahi.toFixed(1) : '-').padStart(5),
        ev,
      ].join('  '),
    );
  }
  if (onlyNight && rows[0]) {
    const r = rows[0];
    console.log(`\nNight ${r.date}: ${r.recordCount} records (${r.invalidRecords} invalid), ${r.usageMinutes} min in ${r.sessions.length} active interval(s)`);
    for (const x of r.sessions) console.log(`  ${x.start.slice(11)} - ${x.end.slice(11)}  ${x.minutes} min`);
    if (r.session) {
      const s = r.session;
      console.log(
        `bkp summary: session ${s.sessionNumber}, usage ${s.usageMinutes} min, start pressure ${s.startPressure}, mean/P90 ${s.pressure.mean}/${s.pressure.p90}, max ${s.observedMaxPressure} cmH2O, large leak ${s.largeLeakMinutes} min, SNI ${s.snoreIndex}`,
      );
      console.log(
        `  AHI ${s.ahi}, apnea index ${s.apneaIndex} (central ${s.centralApneaIndex}, obstructive ${s.obstructiveApneaIndex}), hypopnea index ${s.hypopneaIndex}`,
      );
      console.log(`  leak (slot 384) median ${r.leakMedian}, leak flow (slots 270-319) median ${r.leakFlowMedian} L/min`);
      for (const e of detail[0]!.events)
        console.log(`  ${e.time.iso.slice(11)}  ${e.eventType} (0x${e.code.toString(16).toUpperCase()}) ${e.pressure ?? e.value}`);
    } else console.log('No matching bkp session summary (older than the ring buffer, or no bkp).');
  }
  if (bkp && !onlyNight) console.log(`\nbkp: ${bkp.sessions.length} session summaries, ${bkp.events.length} events, ${bkp.counters.length} counter records, ${bkp.stats.unknownBlocks} unknown blocks`);
  console.error(`(${elapsed}s)`);
}
