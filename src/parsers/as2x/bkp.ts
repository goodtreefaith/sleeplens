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

const EVENT_CODES: Record<number, TherapyEventType> = {
  0xe0: 'pressureChange',
  0xe1: 'apnea1',
  0xe2: 'apnea2',
  0xe3: 'hypopnea',
  0xe8: 'level',
  0xee: 'sessionMarker',
  0xe7: 'sessionMarker',
  0xef: 'sessionMarker',
};

const PRESSURE_EVENTS = new Set<TherapyEventType>(['pressureChange', 'apnea1', 'apnea2', 'hypopnea']);

/** Decode one 8-byte therapy entry: `[yy][ss][MM][dd][code][hh][mi][value]`. */
export function parseTherapyEntry(buf: Uint8Array, offset: number): TherapyEvent | null {
  const yy = buf[offset]!;
  const code = buf[offset + 4]!;
  if (code < 0xe0) return null; // empty (0xFF year handled below) or not an entry
  if (yy === 0xff) return null;
  const time = makeLocalTime(
    2000 + yy,
    buf[offset + 2]!,
    buf[offset + 3]!,
    buf[offset + 5]!,
    buf[offset + 6]!,
    buf[offset + 1]!,
  );
  if (!time) return null;
  const eventType = EVENT_CODES[code] ?? 'unknown';
  const value = buf[offset + 7]!;
  const ev: TherapyEvent = { time, eventType, code, value };
  if (PRESSURE_EVENTS.has(eventType)) ev.pressure = value / 2;
  // Provisional labels (Likely): E1 obstructive, E2 central.
  if (eventType === 'apnea1') ev.likelyKind = 'obstructive';
  else if (eventType === 'apnea2') ev.likelyKind = 'central';
  return ev;
}

function parseSessionSummary(buf: Uint8Array, dv: DataView, o: number): SessionSummary | null {
  const start = readTimestamp(dv, o + 6);
  if (!start) return null;
  const u16 = (rel: number) => dv.getUint16(o + rel, true);
  return {
    sessionNumber: u16(4),
    start,
    usageMinutes: u16(0x54),
    pressure: { min: u16(0x4c) / 10, mean: u16(0x4e) / 10, p90: u16(0x50) / 10 },
    observedMaxPressure: u16(0x5e) / 10,
    ahi: u16(0x58) / 10,
    apneaIndex: u16(0x5a) / 10,
    apnea2Index: u16(0x5c) / 10,
    hypopneaIndex: u16(0x60) / 10,
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
