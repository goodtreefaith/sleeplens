/**
 * Fully synthetic AS2X card. Fake serial, fake 2030 dates, invented values.
 * Pure (no Node APIs) so tests can build it in memory.
 */
import { DAT_HEADER_SIZE, DAT_RECORD_SIZE, SLOT } from '../../src/parsers/as2x/dat';

export const SYNTH_SERIAL = 'TEST0000';

export interface SynthEvent {
  /** Record index the event is stamped at (the END of the event). */
  at: number;
  kind: 'obstructiveApnea' | 'centralApnea' | 'hypopnea';
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
  /** Value of the E8 level entry (0-14 observed on real cards). */
  level: number;
  /** A mid-night pause: records from `pauseAt` on are stamped `pauseSeconds` later. */
  pause?: { at: number; seconds: number };
  /** Snore index x10 for the B5 summary (0x56). */
  snoreIndexX10: number;
  /** Seconds from `at` with leak flow (slots 270-319) at 95 L/min, above the 90 L/min large-leak line. */
  largeLeak?: { at: number; seconds: number };
  /** Extra bkp-only entries: [recordIndex, code, value] (E4-E7 and similar). */
  extraEvents: Array<[number, number, number]>;
}

export const SYNTH_NIGHTS: SynthNight[] = [
  {
    start: { y: 2030, mo: 1, d: 15, h: 22, mi: 30, s: 0 },
    records: 600,
    pressureSteps: [[0, 5.0], [120, 5.5], [240, 6.5], [360, 7.0], [480, 8.0]],
    leakBase: 20,
    leakMod: 10,
    events: [
      { at: 130, kind: 'obstructiveApnea', duration: 15 },
      { at: 250, kind: 'centralApnea', duration: 12 },
      { at: 420, kind: 'hypopnea', duration: 20 },
    ],
    sessionNumber: 1,
    skipAt: 341,
    level: 2,
    snoreIndexX10: 12,
    largeLeak: { at: 450, seconds: 90 }, // 1.5 large-leak minutes
    extraEvents: [
      [140, 0xe4, 3], // snore level
      [150, 0xe7, 5], // snore count
      [380, 0xe6, 2], // flow limitation
      [540, 0xe5, 1], // large leak, stamped at the end of the window
    ],
  },
  {
    start: { y: 2030, mo: 1, d: 16, h: 23, mi: 5, s: 0 },
    records: 720,
    pressureSteps: [[0, 5.0], [180, 6.0], [360, 7.5], [540, 5.5]],
    leakBase: 30,
    leakMod: 12,
    events: [
      { at: 200, kind: 'obstructiveApnea', duration: 14 },
      { at: 500, kind: 'hypopnea', duration: 18 },
    ],
    sessionNumber: 2,
    skipAt: Number.MAX_SAFE_INTEGER, // no drift skip on this night
    level: 14,
    snoreIndexX10: 0,
    extraEvents: [[600, 0xe6, 1]],
  },
  {
    // Starts at second 8 so the first bkp entry of the night disguises itself as a u16 year (2078).
    start: { y: 2030, mo: 1, d: 17, h: 21, mi: 15, s: 8 },
    records: 600,
    pressureSteps: [[0, 5.0], [200, 6.5], [450, 7.0]],
    leakBase: 25,
    leakMod: 8,
    events: [
      { at: 100, kind: 'obstructiveApnea', duration: 14 },
      { at: 250, kind: 'centralApnea', duration: 12 },
      { at: 400, kind: 'centralApnea', duration: 12 },
      { at: 500, kind: 'hypopnea', duration: 16 },
    ],
    sessionNumber: 3,
    skipAt: 100,
    level: 9,
    pause: { at: 300, seconds: 1800 }, // 30 minute pause
    snoreIndexX10: 35,
    extraEvents: [[450, 0xe4, 2]],
  },
];

/** Counter blocks: [year, month, day, counterA, counterB]. Counter A resets to 0 on a filter reset. */
const COUNTERS: Array<Array<[number, number, number, number, number] | null>> = [
  [[2030, 1, 15, 1000, 5000], [2030, 1, 16, 1200, 5200]],
  [[2030, 1, 17, 0, 5400], null],
];

const TEXT_LINES: Array<{ night: number; at: number; text: string }> = [
  { night: 0, at: -5, text: 'SD card Insert' },
  { night: 0, at: 0, text: 'Parameter change Humidifire 15 to 2' },
  { night: 0, at: 599, text: 'Stop work by Auto Off in WorkScreen' },
  { night: 2, at: 0, text: 'Screen Filter to Reset' },
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

/** Timestamp of record `i`, including the one-second clock-drift skip and any pause. */
export function recordStamp(n: SynthNight, i: number): Stamp {
  const pause = n.pause && i >= n.pause.at ? n.pause.seconds : 0;
  return addSec(n.start, i + (i >= n.skipAt ? 1 : 0) + pause);
}

/** Active (therapy) seconds of a night: wall clock minus pauses, drift skips included. */
export function activeSeconds(n: SynthNight): number {
  return n.records + (n.skipAt < n.records ? 1 : 0);
}

/** Round half up, as the device does for its per-hour indices (x10). */
export function indexX10(count: number, usageMinutes: number): number {
  return Math.floor((count * 600) / usageMinutes + 0.5);
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
  b[2] = 0x01; // mode: Auto CPAP
  dv.setUint16(3, 50, true); // 5.0 start pressure
  dv.setUint16(5, 60, true); // 6.0 fixed (CPAP-mode) pressure
  dv.setUint16(7, 45, true); // 4.5 minimum pressure (differs from start so the mapping is tested)
  dv.setUint16(9, 80, true); // 8.0 maximum pressure
  b[0x14] = 3; // comfort (AS-Elex) time
  b[0x15] = 0x22; // comfort level = low nibble (2)
  b[0x16] = 1; // leak alert on
  b[0x17] = 4; // humidifier level
  b[0x18] = 15; // ramp minutes
  b[0x19] = 1; // auto on
  b[0x1a] = 0; // auto off disabled
  b[0x1d] = 0; // pressure unit (raw)
  b[0x20] = 0x01;
  putStamp(dv, 0x21, st);
  b[0x2e] = 3; // constant 3 on real cards; NOT the humidifier level
  b[0x7f] = 0x88;
  return b;
}

function buildDat(n: SynthNight, seed: number): Uint8Array {
  const out = new Uint8Array(DAT_HEADER_SIZE + n.records * DAT_RECORD_SIZE);
  const dv = new DataView(out.buffer);
  // Header: created time, id string, settings block, 0xFF padding.
  putStamp(dv, 0, n.start);
  out[8] = 2; // record format version 2 (812-byte MM records)
  const id = `AS2X${SYNTH_SERIAL}`;
  for (let k = 0; k < id.length; k++) out[0x50 + k] = id.charCodeAt(k);
  out[0x68] = 1; // device type
  out.set([0, 0, 1, 9], 0x69); // software version (invented), least significant byte first -> "9.01.00.00"
  out.set([0, 2, 0, 0], 0x6d); // hardware version -> "2.00"
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
      put(SLOT.pressureFeedback + k, setP + 3 + breath * 2);
      put(SLOT.maskPressure + k, setP - 1 + breath * 3);
      put(SLOT.snore + k, Math.max(0, 40 + breath * 30 * scale[i]!));
      put(SLOT.patientFlow + k, 500 + flow + noise);
      // Leak flow: whole L/min per second, +-1 alternating so the per-second mean is exact.
      const big = n.largeLeak && i >= n.largeLeak.at && i < n.largeLeak.at + n.largeLeak.seconds;
      put(SLOT.leakFlow + k, (big ? 95 : n.leakBase - 10 + (i % n.leakMod)) + (k % 2 ? 1 : -1));
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

const EVENT_CODE = { obstructiveApnea: 0xe1, centralApnea: 0xe2, hypopnea: 0xe3 } as const;

function entry(st: Stamp, code: number, value: number): Uint8Array {
  return Uint8Array.of(st.y - 2000, st.s, st.mo, st.d, code, st.h, st.mi, value);
}

function buildBkp(): Uint8Array {
  const blocks: Uint8Array[] = [];

  // Opening settings snapshots, one per night.
  for (const n of SYNTH_NIGHTS) blocks.push(settingsBlock(n.start));

  // Event blocks, chunked per night (like separate ring-buffer writes).
  SYNTH_NIGHTS.forEach((n, ni) => {
    const entries: Uint8Array[] = [];
    entries.push(entry(recordStamp(n, 0), 0xee, 0)); // treatment start
    for (const [at, p] of n.pressureSteps) entries.push(entry(recordStamp(n, at), 0xe0, Math.round(p * 2)));
    entries.push(entry(recordStamp(n, 300 + ni * 10), 0xe8, n.level)); // level
    for (const ev of n.events) {
      entries.push(entry(recordStamp(n, ev.at), EVENT_CODE[ev.kind], Math.round(pressureAt(n, ev.at) * 2)));
    }
    for (const [at, code, value] of n.extraEvents) entries.push(entry(recordStamp(n, at), code, value));
    entries.push(entry(recordStamp(n, n.records - 1), 0xef, 0)); // treatment end
    entries.sort((a, b) => cmpEntry(a, b));
    for (let i = 0; i < entries.length; i += 16) {
      const b = new Uint8Array(128).fill(0xff);
      entries.slice(i, i + 16).forEach((e, k) => b.set(e, k * 8));
      blocks.push(b);
    }
  });

  // Counter blocks: two 64-byte records each; A = time since filter reset, B cumulative, C = B - 1.
  for (const pair of COUNTERS) {
    const b = new Uint8Array(128).fill(0xff);
    pair.forEach((rec, k) => {
      if (!rec) return;
      const r = new Uint8Array(64);
      const dv = new DataView(r.buffer);
      dv.setUint16(0, rec[0], true);
      r[2] = rec[1];
      r[3] = rec[2];
      dv.setUint32(4, rec[3], true);
      dv.setUint32(8, rec[4], true);
      dv.setUint32(12, rec[4] - 1, true);
      r[0x14] = 1;
      r[0x3f] = 0x88;
      b.set(r, k * 64);
    });
    blocks.push(b);
  }

  // Session summaries (A5A5).
  for (const n of SYNTH_NIGHTS) {
    const b = new Uint8Array(128);
    const dv = new DataView(b.buffer);
    const usageMinutes = Math.ceil(activeSeconds(n) / 60);
    const count = (...kinds: string[]) => n.events.filter((e) => kinds.includes(e.kind)).length;
    const mean = n.pressureSteps.reduce((sum, [at, p], k) => {
      const next = n.pressureSteps[k + 1]?.[0] ?? n.records;
      return sum + p * (next - at);
    }, 0) / n.records;
    const sorted: number[] = [];
    for (let i = 0; i < n.records; i++) sorted.push(Math.round(pressureAt(n, i) * 10));
    sorted.sort((a, c) => a - c);
    // Coarse P90 and observed max, stored in 0.5 cmH2O steps (Likely).
    const coarse = (tenths: number) => Math.round(tenths / 5) * 5;
    const p90 = coarse(sorted[Math.ceil(0.9 * n.records) - 1]!);
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
    dv.setUint16(0x4c, Math.round(n.pressureSteps[0]![1] * 10), true); // start pressure
    dv.setUint16(0x52, Math.round(((n.largeLeak?.seconds ?? 0) / 60) * 10), true); // large-leak minutes x10
    dv.setUint16(0x56, n.snoreIndexX10, true); // snore index x10
    dv.setUint16(0x4e, Math.round(mean * 10), true);
    dv.setUint16(0x50, p90, true);
    // The device rounds usage (sum of active intervals) up to whole minutes (Likely).
    dv.setUint16(0x54, usageMinutes, true);
    dv.setUint16(0x58, indexX10(count('obstructiveApnea', 'centralApnea', 'hypopnea'), usageMinutes), true);
    dv.setUint16(0x5a, indexX10(count('obstructiveApnea', 'centralApnea'), usageMinutes), true);
    dv.setUint16(0x5c, indexX10(count('centralApnea'), usageMinutes), true);
    dv.setUint16(0x5e, coarse(sorted[n.records - 1]!), true);
    dv.setUint16(0x60, indexX10(count('hypopnea'), usageMinutes), true);
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
