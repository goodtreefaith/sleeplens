import type {
  BkpParseResult,
  BkpStats,
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
    pressure: { min: u16(0x4c) / 10, mean: u16(0x4e) / 10, p95: u16(0x50) / 10 },
    maxPressureSetting: u16(0x5e) / 10,
    ahi: u16(0x58) / 10,
    apneaIndex: u16(0x5a) / 10,
    hypopneaIndex: u16(0x5c) / 10,
    hasB5Marker: buf[o + 0x40] === 0xb5 && buf[o + 0x41] === 0xb5,
  };
}

/** Parse a whole `<serial>.bkp` log. */
export function parseBkp(buf: Uint8Array): BkpParseResult {
  const dv = makeDataView(buf);
  const settings: SettingsSnapshot[] = [];
  const sessions: SessionSummary[] = [];
  const events: TherapyEvent[] = [];
  const textEvents: TextEvent[] = [];
  const stats: BkpStats = { blocks: 0, emptyBlocks: 0, unknownBlocks: 0, skippedEntries: 0 };

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

    const year = b0 | (b1 << 8);
    if (year >= 2000 && year <= 2100) {
      // Text event line: timestamp, 0xFF, ASCII up to NUL.
      const time = readTimestamp(dv, o);
      if (time && buf[o + 7] === 0xff) {
        textEvents.push({ time, text: readAscii(buf, o + 8, BLOCK_SIZE - 8) });
      } else {
        stats.unknownBlocks++;
      }
      continue;
    }

    // Therapy block: first byte is a 2-digit year, second byte a second (0-59).
    if (b0 >= 10 && b0 <= 99 && b1 < 60) {
      for (let e = 0; e < BLOCK_SIZE; e += 8) {
        const ev = parseTherapyEntry(buf, o + e);
        if (ev) events.push(ev);
        else stats.skippedEntries++;
      }
      continue;
    }
    stats.unknownBlocks++;
  }

  events.sort((a, b) => a.time.epochSec - b.time.epochSec);
  settings.sort((a, b) => (a.time?.epochSec ?? 0) - (b.time?.epochSec ?? 0));
  sessions.sort((a, b) => (a.start?.epochSec ?? 0) - (b.start?.epochSec ?? 0));
  textEvents.sort((a, b) => a.time.epochSec - b.time.epochSec);
  return { settings, sessions, events, textEvents, stats };
}
