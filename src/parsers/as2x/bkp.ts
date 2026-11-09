import type {
  BkpParseResult,
  BkpStats,
  CounterRecord,
  SessionSummary,
  SettingsSnapshot,
  TextEvent,
  TherapyEvent,
  TherapyEventType,
} from './types';
import { BLOCK_SIZE, parseSettingsBlock } from './settings';
import { makeDataView, makeLocalTime, readAscii, readTimestamp } from './time';

/**
 * Event code (low nibble) to type. The same table serves NeoRead's 0xA0-0xAF
 * codes and the newer firmware's 0xE0-0xEF codes. See TherapyEventType for
 * the confidence of each.
 */
const EVENT_NIBBLES: Record<number, TherapyEventType> = {
  0x0: 'pressureChange',
  0x1: 'obstructiveApnea',
  0x2: 'centralApnea',
  0x3: 'hypopnea',
  0x4: 'snoreLevel',
  0x5: 'largeLeak',
  0x6: 'flowLimitation',
  0x7: 'snoreCount',
  0xe: 'treatmentStart',
  0xf: 'treatmentEnd',
};

/** Event type for a code byte, or null when the code is outside 0xA0-0xAF and 0xE0-0xEF. */
export function eventTypeForCode(code: number): TherapyEventType | null {
  const hi = code & 0xf0;
  if (hi !== 0xa0 && hi !== 0xe0) return null;
  if (code === 0xe8) return 'level'; // no NeoRead equivalent (Unresolved)
  return EVENT_NIBBLES[code & 0x0f] ?? 'unknown';
}

const PRESSURE_EVENTS = new Set<TherapyEventType>(['pressureChange', 'obstructiveApnea', 'centralApnea', 'hypopnea']);

/**
 * Decode one 8-byte therapy entry: `[yy][ss][MM][dd][code][hh][mi][value]`
 * (firmware 01.01.19 and later). Codes 0xA0-0xAF and 0xE0-0xEF only.
 *
 * Older firmware stores a u16 year in bytes 0-1 and no seconds. That form is
 * recognised when byte 1 is 0x07 and byte 0 is 0xD0 or more (years 2000-2047),
 * which the newer layout cannot produce (it would mean year 2208 or later).
 */
export function parseTherapyEntry(buf: Uint8Array, offset: number): TherapyEvent | null {
  const b0 = buf[offset]!;
  const b1 = buf[offset + 1]!;
  const code = buf[offset + 4]!;
  if (b0 === 0xff) return null; // empty entry
  const eventType = eventTypeForCode(code);
  if (!eventType) return null;
  const u16Year = b1 === 0x07 && b0 >= 0xd0;
  const time = makeLocalTime(
    u16Year ? b0 | (b1 << 8) : 2000 + b0,
    buf[offset + 2]!,
    buf[offset + 3]!,
    buf[offset + 5]!,
    buf[offset + 6]!,
    u16Year ? 0 : b1,
  );
  if (!time) return null;
  const value = buf[offset + 7]!;
  const ev: TherapyEvent = { time, eventType, code, value };
  if (PRESSURE_EVENTS.has(eventType)) ev.pressure = value / 2;
  return ev;
}

const round1 = (x: number) => Math.round(x * 10) / 10;

function parseSessionSummary(buf: Uint8Array, dv: DataView, o: number): SessionSummary | null {
  const start = readTimestamp(dv, o + 6);
  if (!start) return null;
  const u16 = (rel: number) => dv.getUint16(o + rel, true);
  // NeoRead reads 0xFFFF in these four fields as 0.
  const idx = (rel: number) => (u16(rel) === 0xffff ? 0 : u16(rel) / 10);
  const startPressure = dv.getInt16(o + 0x4c, true) / 10;
  const apneaIndex = idx(0x5a);
  const centralApneaIndex = idx(0x5c);
  return {
    sessionNumber: u16(4),
    start,
    usageMinutes: u16(0x54),
    startPressure,
    pressure: { min: startPressure, mean: u16(0x4e) / 10, p90: u16(0x50) / 10 },
    observedMaxPressure: idx(0x5e),
    largeLeakMinutes: u16(0x52) / 10,
    snoreIndex: u16(0x56) / 10,
    ahi: u16(0x58) / 10,
    apneaIndex,
    centralApneaIndex,
    apnea2Index: centralApneaIndex,
    obstructiveApneaIndex: Math.max(0, round1(apneaIndex - centralApneaIndex)),
    hypopneaIndex: idx(0x60),
    hasB5Marker: buf[o + 0x40] === 0xb5 && buf[o + 0x41] === 0xb5,
  };
}

const COUNTER_RECORD_SIZE = 64;

/** Counter record at `o`: null if empty (all 0xFF or all 0x00), undefined if malformed. */
function parseCounterRecord(buf: Uint8Array, dv: DataView, o: number): CounterRecord | null | undefined {
  let allFF = true;
  let all00 = true;
  for (let i = 0; i < COUNTER_RECORD_SIZE; i++) {
    if (buf[o + i] !== 0xff) allFF = false;
    if (buf[o + i] !== 0x00) all00 = false;
  }
  if (allFF || all00) return null;
  if (buf[o + 0x3f] !== 0x88) return undefined;
  const year = dv.getUint16(o, true);
  const month = buf[o + 2]!;
  const day = buf[o + 3]!;
  if (year < 2000 || year > 2100 || month < 1 || month > 12 || day < 1 || day > 31) return undefined;
  const counterA = dv.getUint32(o + 4, true);
  const counterB = dv.getUint32(o + 8, true);
  const counterC = dv.getUint32(o + 12, true);
  if (counterC !== counterB - 1) return undefined;
  const p = (x: number, w = 2) => String(x).padStart(w, '0');
  return { date: `${p(year, 4)}-${p(month)}-${p(day)}`, year, month, day, counterA, counterB, counterC };
}

/** A counter block is two valid or empty records, at least one populated. */
function parseCounterBlock(buf: Uint8Array, dv: DataView, o: number): CounterRecord[] | null {
  const out: CounterRecord[] = [];
  for (let r = 0; r < BLOCK_SIZE; r += COUNTER_RECORD_SIZE) {
    const rec = parseCounterRecord(buf, dv, o + r);
    if (rec === undefined) return null;
    if (rec) out.push(rec);
  }
  return out.length > 0 ? out : null;
}

/**
 * An event block is 16 entries; each is empty (0xFF year) or fully valid.
 * Returns the parsed events, or null when any entry is malformed or none are valid.
 */
function parseEventBlock(buf: Uint8Array, o: number): { events: TherapyEvent[]; skipped: number } | null {
  const events: TherapyEvent[] = [];
  let skipped = 0;
  for (let e = 0; e < BLOCK_SIZE; e += 8) {
    const yy = buf[o + e]!;
    const ev = parseTherapyEntry(buf, o + e);
    if (ev) events.push(ev);
    else if (yy === 0xff) skipped++;
    else return null;
  }
  return events.length > 0 ? { events, skipped } : null;
}

/** Parse a whole `<serial>.bkp` log. */
export function parseBkp(buf: Uint8Array): BkpParseResult {
  const dv = makeDataView(buf);
  const settings: SettingsSnapshot[] = [];
  const sessions: SessionSummary[] = [];
  const events: TherapyEvent[] = [];
  const textEvents: TextEvent[] = [];
  const counters: CounterRecord[] = [];
  const stats: BkpStats = { blocks: 0, emptyBlocks: 0, unknownBlocks: 0, counterBlocks: 0, skippedEntries: 0 };

  const blockCount = Math.floor(buf.length / BLOCK_SIZE);
  for (let b = 0; b < blockCount; b++) {
    const o = b * BLOCK_SIZE;
    stats.blocks++;
    const b0 = buf[o]!;
    const b1 = buf[o + 1]!;

    if (b0 === 0xff && b1 === 0xff) {
      stats.emptyBlocks++;
      continue;
    }
    if (b0 === 0xc5 && b1 === 0xc5) {
      const s = parseSettingsBlock(buf, o);
      if (s) settings.push(s);
      else stats.unknownBlocks++;
      continue;
    }
    if (b0 === 0xa5 && b1 === 0xa5) {
      const s = parseSessionSummary(buf, dv, o);
      if (s) sessions.push(s);
      else stats.unknownBlocks++;
      continue;
    }

    // Remaining types are told apart by their complete structure, not by the
    // first bytes: an event block whose first entry has second = 8 starts with
    // bytes that read as a u16 year (2073 / 2074).
    const counterRecs = parseCounterBlock(buf, dv, o);
    if (counterRecs) {
      counters.push(...counterRecs);
      stats.counterBlocks++;
      continue;
    }

    // Text event line: timestamp, 0xFF, ASCII up to NUL.
    const time = readTimestamp(dv, o);
    if (time && buf[o + 7] === 0xff) {
      textEvents.push({ time, text: readAscii(buf, o + 8, BLOCK_SIZE - 8) });
      continue;
    }

    // Therapy block: 16 entries `[yy][ss][MM][dd][code][hh][mi][value]`.
    const evBlock = parseEventBlock(buf, o);
    if (evBlock) {
      events.push(...evBlock.events);
      stats.skippedEntries += evBlock.skipped;
      continue;
    }
    stats.unknownBlocks++;
  }

  events.sort((a, b) => a.time.epochSec - b.time.epochSec);
  settings.sort((a, b) => (a.time?.epochSec ?? 0) - (b.time?.epochSec ?? 0));
  sessions.sort((a, b) => (a.start?.epochSec ?? 0) - (b.start?.epochSec ?? 0));
  textEvents.sort((a, b) => a.time.epochSec - b.time.epochSec);
  counters.sort((a, b) => (a.date < b.date ? -1 : a.date > b.date ? 1 : a.counterB - b.counterB));
  return { settings, sessions, events, textEvents, counters, stats };
}
