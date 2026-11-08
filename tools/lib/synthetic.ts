/**
 * Fully synthetic AS2X card. Fake serial, fake 2030 dates, invented values.
 * Pure (no Node APIs) so tests can build it in memory.
 */
import { DAT_HEADER_SIZE, DAT_RECORD_SIZE, SLOT } from '../../src/parsers/as2x/dat';

export const SYNTH_SERIAL = 'TEST0000';

export interface SynthEvent {
  /** Record index the event is stamped at (the END of the event). */
  at: number;
  kind: 'apnea1' | 'apnea2' | 'hypopnea';
  /** Seconds of reduced flow before the stamp. */
  duration: number;
}

export interface SynthNight {
  /** Fake start, year 2030. */
  start: { y: number; mo: number; d: number; h: number; mi: number; s: number };
  records: number;
  /** [recordIndex, cmH2O] pressure steps, ascending. */
  pressureSteps: Array<[number, number]>;
  /** Leak is `leakBase + (i % leakMod)`. */
  leakBase: number;
  leakMod: number;
  events: SynthEvent[];
  sessionNumber: number;
  /** One timestamp skip (clock drift) at this record index. */
  skipAt: number;
}

export const SYNTH_NIGHTS: SynthNight[] = [
  {
    start: { y: 2030, mo: 1, d: 15, h: 22, mi: 30, s: 0 },
    records: 600,
    pressureSteps: [[0, 5.0], [120, 5.5], [240, 6.5], [360, 7.0], [480, 8.0]],
    leakBase: 20,
    leakMod: 10,
    events: [
      { at: 130, kind: 'apnea1', duration: 15 },
      { at: 250, kind: 'apnea2', duration: 12 },
      { at: 420, kind: 'hypopnea', duration: 20 },
    ],
    sessionNumber: 1,
    skipAt: 341,
  },
  {
    start: { y: 2030, mo: 1, d: 16, h: 23, mi: 5, s: 0 },
    records: 720,
    pressureSteps: [[0, 5.0], [180, 6.0], [360, 7.5], [540, 5.5]],
    leakBase: 30,
    leakMod: 12,
    events: [
      { at: 200, kind: 'apnea1', duration: 14 },
      { at: 500, kind: 'hypopnea', duration: 18 },
    ],
    sessionNumber: 2,
    skipAt: Number.MAX_SAFE_INTEGER, // no drift skip on this night
  },
];

const TEXT_LINES: Array<{ night: number; at: number; text: string }> = [
  { night: 0, at: -5, text: 'SD card Insert' },
  { night: 0, at: 0, text: 'Parameter change Humidifire 15 to 2' },
  { night: 0, at: 599, text: 'Stop work by Auto Off in WorkScreen' },
  { night: 1, at: 0, text: 'Machine Reset = 6  Software Version TEST-0.00' },
];

function mulberry32(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type Stamp = { y: number; mo: number; d: number; h: number; mi: number; s: number };

function addSec(st: Stamp, sec: number): Stamp {
  const ms = Date.UTC(st.y, st.mo - 1, st.d, st.h, st.mi, st.s) + sec * 1000;
  const d = new Date(ms);
  return {
    y: d.getUTCFullYear(),
    mo: d.getUTCMonth() + 1,
    d: d.getUTCDate(),
    h: d.getUTCHours(),
    mi: d.getUTCMinutes(),
    s: d.getUTCSeconds(),
  };
}

function weekday(st: Stamp): number {
  return (new Date(Date.UTC(st.y, st.mo - 1, st.d)).getUTCDay() + 6) % 7; // Monday = 0
}

/** Timestamp of record `i`, including the one-second clock-drift skip. */
export function recordStamp(n: SynthNight, i: number): Stamp {
  return addSec(n.start, i + (i >= n.skipAt ? 1 : 0));
}

function putStamp(dv: DataView, o: number, st: Stamp) {
  dv.setUint16(o, st.y, true);
  dv.setUint8(o + 2, st.mo);
  dv.setUint8(o + 3, st.d);
  dv.setUint8(o + 4, st.h);
  dv.setUint8(o + 5, st.mi);
  dv.setUint8(o + 6, st.s);
}

function pressureAt(n: SynthNight, i: number): number {
  let p = n.pressureSteps[0]![1];
  for (const [at, v] of n.pressureSteps) if (i >= at) p = v;
  return p;
}

function settingsBlock(st: Stamp): Uint8Array {
  const b = new Uint8Array(128);
  const dv = new DataView(b.buffer);
  b[0] = 0xc5;
  b[1] = 0xc5;
  b[2] = 0x01;
  dv.setUint16(3, 0x32, true); // 5.0 min
  dv.setUint16(5, 0x3c, true);
  dv.setUint16(7, 0x32, true);
  dv.setUint16(9, 0x50, true); // 8.0 max
  b[0x20] = 0x01;
  putStamp(dv, 0x21, st);
  b[0x2e] = 3; // humidifier level
  return b;
}

function buildDat(n: SynthNight, seed: number): Uint8Array {
  const out = new Uint8Array(DAT_HEADER_SIZE + n.records * DAT_RECORD_SIZE);
  const dv = new DataView(out.buffer);
  // Header: created time, id string, settings block, 0xFF padding.
  putStamp(dv, 0, n.start);
  const id = `AS2X${SYNTH_SERIAL}`;
  for (let k = 0; k < id.length; k++) out[0x50 + k] = id.charCodeAt(k);
  out.set(settingsBlock(n.start), 0x140);
  out.fill(0xff, 0x1c0, DAT_HEADER_SIZE);

  const rand = mulberry32(seed);
  // Flat-flow windows: record index -> amplitude scale.
  const scale = new Float32Array(n.records).fill(1);
  for (const ev of n.events) {
    const f = ev.kind === 'hypopnea' ? 0.3 : 0.04;
    for (let i = Math.max(0, ev.at - ev.duration); i < ev.at; i++) scale[i] = f;
  }

  for (let i = 0; i < n.records; i++) {
    const base = DAT_HEADER_SIZE + i * DAT_RECORD_SIZE;
    out[base] = 0x4d;
    out[base + 1] = 0x4d;
    const st = recordStamp(n, i);
    putStamp(dv, base + 4, st);
    out[base + 11] = weekday(st);
    const data = base + 12;
    const put = (slot: number, v: number) => dv.setUint16(data + slot * 2, Math.max(0, Math.round(v)), true);

    const setP = Math.round(pressureAt(n, i) * 10);
    for (let k = 0; k < 50; k++) {
      const t = i + k / 50;
      const breath = Math.sin(2 * Math.PI * 0.3 * t);
      const flow = 22 * breath * scale[i]!;
      const noise = (rand() - 0.5) * 2;
      put(SLOT.setPressure + k, setP);
      put(SLOT.blowerPressure + k, setP + 3 + breath * 2);
      put(SLOT.maskPressure + k, setP - 1 + breath * 3);
      put(170 + k, Math.max(0, 40 + breath * 30 * scale[i]!));
      put(SLOT.patientFlow + k, 500 + flow + noise);
      put(270 + k, Math.floor(Math.abs(breath) * 8));
      put(SLOT.totalFlow + k, 500 + flow + n.leakBase + (i % n.leakMod) + noise);
    }
    put(374, 25);
    put(375, 300 + setP / 4);
    put(376, 56);
    put(SLOT.elapsed, i);
    put(380, 50);
    put(SLOT.state, 5);
    put(383, 22);
    put(SLOT.leak, n.leakBase + (i % n.leakMod));
    put(391, 4000 + setP * 40);
  }
  return out;
}

const EVENT_CODE = { apnea1: 0xe1, apnea2: 0xe2, hypopnea: 0xe3 } as const;

function entry(st: Stamp, code: number, value: number): Uint8Array {
  return Uint8Array.of(st.y - 2000, st.s, st.mo, st.d, code, st.h, st.mi, value);
}

function buildBkp(): Uint8Array {
  const blocks: Uint8Array[] = [];
  const entries: Uint8Array[] = [];

  // Opening settings snapshots, one per night.
  for (const n of SYNTH_NIGHTS) blocks.push(settingsBlock(n.start));

  SYNTH_NIGHTS.forEach((n, ni) => {
    entries.push(entry(recordStamp(n, 0), 0xee, 0)); // session start marker
    for (const [at, p] of n.pressureSteps) entries.push(entry(recordStamp(n, at), 0xe0, Math.round(p * 2)));
    entries.push(entry(recordStamp(n, 300 + ni * 10), 0xe8, 2)); // level
    for (const ev of n.events) {
      entries.push(entry(recordStamp(n, ev.at), EVENT_CODE[ev.kind], Math.round(pressureAt(n, ev.at) * 2)));
    }
    entries.push(entry(recordStamp(n, n.records - 1), 0xe7, 0)); // session end marker
  });
  entries.sort((a, b) => cmpEntry(a, b));
  for (let i = 0; i < entries.length; i += 16) {
    const b = new Uint8Array(128).fill(0xff);
    entries.slice(i, i + 16).forEach((e, k) => b.set(e, k * 8));
    blocks.push(b);
  }

  // Session summaries (A5A5).
  for (const n of SYNTH_NIGHTS) {
    const b = new Uint8Array(128);
    const dv = new DataView(b.buffer);
    const hours = n.records / 3600;
    const apneas = n.events.filter((e) => e.kind !== 'hypopnea').length;
    const hypos = n.events.length - apneas;
    const mean = n.pressureSteps.reduce((sum, [at, p], k) => {
      const next = n.pressureSteps[k + 1]?.[0] ?? n.records;
      return sum + p * (next - at);
    }, 0) / n.records;
    const sorted: number[] = [];
    for (let i = 0; i < n.records; i++) sorted.push(Math.round(pressureAt(n, i) * 10));
    sorted.sort((a, c) => a - c);
    const p95 = sorted[Math.ceil(0.95 * n.records) - 1]!;
    b[0] = 0xa5;
    b[1] = 0xa5;
    b[2] = 0x01;
    b[3] = 0x01;
    dv.setUint16(4, n.sessionNumber, true);
    putStamp(dv, 6, n.start);
    b[13] = weekday(n.start);
    b[0x40] = 0xb5;
    b[0x41] = 0xb5;
    putStamp(dv, 0x42, n.start);
    b[0x49] = 0xff;
    dv.setUint16(0x4a, n.sessionNumber, true);
    dv.setUint16(0x4c, sorted[0]!, true);
    dv.setUint16(0x4e, Math.round(mean * 10), true);
    dv.setUint16(0x50, p95, true);
    // The device rounds usage up to whole minutes of wall-clock span (Likely).
    const span = n.records + (n.skipAt < n.records ? 1 : 0);
    dv.setUint16(0x54, Math.ceil(span / 60), true);
    dv.setUint16(0x58, Math.round((10 * n.events.length) / hours), true);
    dv.setUint16(0x5a, Math.round((10 * apneas) / hours), true);
    dv.setUint16(0x5c, Math.round((10 * hypos) / hours), true);
    dv.setUint16(0x5e, 0x50, true);
    dv.setUint16(0x60, 4, true);
    blocks.push(b);
  }

  // C5C5 block mid-log plus text event lines.
  for (const t of TEXT_LINES) {
    const n = SYNTH_NIGHTS[t.night]!;
    const b = new Uint8Array(128).fill(0xff);
    const dv = new DataView(b.buffer);
    putStamp(dv, 0, addSec(recordStamp(n, 0), t.at));
    b[7] = 0xff;
    const text = t.text;
    for (let k = 0; k < text.length; k++) b[8 + k] = text.charCodeAt(k);
    b[8 + text.length] = 0;
    blocks.push(b);
  }

  // A few empty blocks, like the unused tail of the ring buffer.
  for (let k = 0; k < 4; k++) blocks.push(new Uint8Array(128).fill(0xff));

  const out = new Uint8Array(blocks.length * 128);
  blocks.forEach((b, i) => out.set(b, i * 128));
  return out;
}

function cmpEntry(a: Uint8Array, b: Uint8Array): number {
  const key = (e: Uint8Array) =>
    Date.UTC(2000 + e[0]!, e[2]! - 1, e[3]!, e[5]!, e[6]!, e[1]!);
  return key(a) - key(b);
}

/** Relative path -> file bytes for the whole synthetic card. */
export function buildSyntheticCard(): Map<string, Uint8Array> {
  const files = new Map<string, Uint8Array>();
  SYNTH_NIGHTS.forEach((n, i) => {
    const ym = `${n.start.y}${String(n.start.mo).padStart(2, '0')}`;
    const ymd = `${ym}${String(n.start.d).padStart(2, '0')}`;
    files.set(`${SYNTH_SERIAL}${ym}/${ymd}.dat`, buildDat(n, 1000 + i));
  });
  files.set(`${SYNTH_SERIAL}.bkp`, buildBkp());
  return files;
}
