import type {
  ActiveInterval,
  DatHeader,
  DatRecord,
  DatSeries,
  DatSummary,
  DatWaveforms,
  LocalTime,
} from './types';
import { makeDataView, readAscii, readTimestamp } from './time';
import { parseSettingsBlock } from './settings';

export const DAT_HEADER_SIZE = 0x800;
/** Version 2 record size (magic `MM`). The only layout decoded. */
export const DAT_RECORD_SIZE = 812;
/** Version 1 record size (magic `<<`), per NeoRead. Not decoded. */
export const DAT_RECORD_SIZE_V1 = 512;
export const SAMPLES_PER_RECORD = 400;
export const WAVE_SAMPLES = 50;

/** Slot indexes into a record's 400 u16 values (see FORMAT_NOTES). Slot s is at record byte 12 + 2s. */
export const SLOT = {
  setPressure: 20,
  /** NeoRead "PressureFdb": the pressure NeoRead plots. */
  pressureFeedback: 70,
  /** @deprecated alias of `pressureFeedback`. */
  blowerPressure: 70,
  maskPressure: 120,
  /** NeoRead "Snore", raw. */
  snore: 170,
  patientFlow: 220,
  /** NeoRead "LeakFlow", L/min as stored. Looks like EXCESS (unintentional) leak (Likely). */
  leakFlow: 270,
  totalFlow: 320,
  elapsed: 379,
  state: 381,
  /** Looks like TOTAL leak, including the mask's vent flow (Likely). */
  leak: 384,
} as const;

const RECORD_DATA_OFFSET = 12;
const FLOW_OFFSET = 500;

/** NeoRead's model table, keyed by the first three characters of the device ID. */
export const MODEL_NAMES: Readonly<Record<string, string>> = {
  AS1: 'AS100C',
  AS2: 'AS100A',
  AS3: 'AS100A',
  AS4: 'AS100A-R',
  AS5: 'AS100A-RT',
  AS6: 'AS100C',
  AS7: 'AS100C-R',
  AS8: 'AS100C-RT',
};

export function modelNameForId(deviceId: string): string | null {
  return MODEL_NAMES[deviceId.slice(0, 3).toUpperCase()] ?? null;
}

/**
 * Format a 4-byte version (least significant byte first) as NeoRead does:
 * `b3.b2.b1.b0` when b3 != 0, else `b2.b1.b0` when b2 != 0, else `b1.b0`;
 * every part after the first is two digits. Null when the bytes are all 0x00 or all 0xFF.
 */
export function formatDeviceVersion(b: ArrayLike<number>): string | null {
  const v = [b[0] ?? 0, b[1] ?? 0, b[2] ?? 0, b[3] ?? 0];
  if (v.every((x) => x === 0) || v.every((x) => x === 0xff)) return null;
  const parts = v[3] !== 0 ? [v[3], v[2], v[1], v[0]] : v[2] !== 0 ? [v[2], v[1], v[0]] : [v[1], v[0]];
  return parts.map((x, i) => (i === 0 ? String(x) : String(x).padStart(2, '0'))).join('.');
}

export function parseDatHeader(buf: Uint8Array): DatHeader {
  const dv = makeDataView(buf);
  const bodyBytes = Math.max(0, buf.length - DAT_HEADER_SIZE);
  const formatVersion = buf.length > 8 ? buf[8]! : 0;
  const recordsSupported = formatVersion !== 1;
  const recordSize = recordsSupported ? DAT_RECORD_SIZE : DAT_RECORD_SIZE_V1;
  // Device ID: 24 bytes at 0x50 (NeoRead), `AS2X<SERIAL>`.
  const id = buf.length >= 0x60 ? readAscii(buf, 0x50, 24) : '';
  const version = (o: number) => (buf.length >= o + 4 ? formatDeviceVersion(buf.subarray(o, o + 4)) : null);
  return {
    created: readTimestamp(dv, 0),
    formatVersion,
    recordsSupported,
    deviceModel: id.slice(0, 4),
    modelName: modelNameForId(id),
    serial: id.slice(4),
    deviceType: buf.length > 0x68 ? buf[0x68]! : 0,
    softwareVersion: version(0x69),
    hardwareVersion: version(0x6d),
    settings: parseSettingsBlock(buf, 0x140),
    recordCount: Math.floor(bodyBytes / recordSize),
    trailingBytes: bodyBytes % recordSize,
  };
}

export interface IterateOptions {
  /** Also decode the 50 Hz waveforms of each record (allocates per record). Default false. */
  waveforms?: boolean;
}

/**
 * Yield one record per second of therapy. Records are decoded lazily and never
 * retained, so a 30 MB night is processed with constant memory. Version 1
 * files (512-byte records) yield nothing.
 */
export function* iterateDatRecords(
  buf: Uint8Array,
  options: IterateOptions = {},
): Generator<DatRecord> {
  const dv = makeDataView(buf);
  const header = parseDatHeader(buf);
  if (!header.recordsSupported) return;
  const count = header.recordCount;
  for (let i = 0; i < count; i++) {
    const base = DAT_HEADER_SIZE + i * DAT_RECORD_SIZE;
    const magicOk = buf[base] === 0x4d && buf[base + 1] === 0x4d;
    const time = magicOk ? readTimestamp(dv, base + 4) : null;
    const data = base + RECORD_DATA_OFFSET;
    const u16 = (slot: number) => dv.getUint16(data + slot * 2, true);
    // NeoRead reads snore and leak flow as signed 16-bit.
    const i16 = (slot: number) => dv.getInt16(data + slot * 2, true);

    let setSum = 0;
    let maskSum = 0;
    let snoreSum = 0;
    let leakFlowSum = 0;
    let flowMin = 65535;
    let flowMax = 0;
    for (let k = 0; k < WAVE_SAMPLES; k++) {
      setSum += u16(SLOT.setPressure + k);
      maskSum += u16(SLOT.maskPressure + k);
      snoreSum += i16(SLOT.snore + k);
      leakFlowSum += i16(SLOT.leakFlow + k);
      const f = u16(SLOT.patientFlow + k);
      if (f < flowMin) flowMin = f;
      if (f > flowMax) flowMax = f;
    }

    const rec: DatRecord = {
      index: i,
      time,
      weekday: buf[base + 11]!,
      valid: time !== null,
      setPressure: setSum / WAVE_SAMPLES / 10,
      maskPressure: maskSum / WAVE_SAMPLES / 10,
      flowAmplitude: flowMax - flowMin,
      totalLeak: u16(SLOT.leak),
      leak: u16(SLOT.leak),
      excessLeak: leakFlowSum / WAVE_SAMPLES,
      leakFlow: leakFlowSum / WAVE_SAMPLES,
      snore: snoreSum / WAVE_SAMPLES,
      elapsedCounter: u16(SLOT.elapsed),
      stateCode: u16(SLOT.state),
    };
    if (options.waveforms) rec.waveforms = readWaveforms(dv, data);
    yield rec;
  }
}

function readWaveforms(dv: DataView, data: number): DatWaveforms {
  const read = (slot: number, scale: (v: number) => number, signed = false) => {
    const out = new Float32Array(WAVE_SAMPLES);
    for (let k = 0; k < WAVE_SAMPLES; k++) {
      const at = data + (slot + k) * 2;
      out[k] = scale(signed ? dv.getInt16(at, true) : dv.getUint16(at, true));
    }
    return out;
  };
  const pressure = (v: number) => v / 10;
  const flow = (v: number) => v - FLOW_OFFSET;
  const raw = (v: number) => v;
  return {
    setPressure: read(SLOT.setPressure, pressure),
    pressureFeedback: read(SLOT.pressureFeedback, pressure),
    maskPressure: read(SLOT.maskPressure, pressure),
    snore: read(SLOT.snore, raw, true),
    patientFlow: read(SLOT.patientFlow, flow),
    leakFlow: read(SLOT.leakFlow, raw, true),
    totalFlow: read(SLOT.totalFlow, flow),
  };
}

/** Nearest-rank percentile (0-1) over a histogram: smallest bin whose cumulative count reaches ceil(p * n). */
function histPercentile(hist: Uint32Array, total: number, p: number): number {
  if (total === 0) return 0;
  const rank = Math.max(1, Math.ceil(p * total - 1e-9));
  let cum = 0;
  for (let v = 0; v < hist.length; v++) {
    cum += hist[v]!;
    if (cum >= rank) return v;
  }
  return hist.length - 1;
}

/** A timestamp gap above this many seconds ends an active interval. Smaller gaps are clock drift. */
export const PAUSE_GAP_SECONDS = 60;

export interface SummarizeOptions {
  /** Keep a 1 Hz series per channel. Default true. */
  includeSeries?: boolean;
}

/** One streaming pass over a night file: usage, pressure and leak statistics, 1 Hz series. */
export function summarizeDat(buf: Uint8Array, options: SummarizeOptions = {}): DatSummary {
  const header = parseDatHeader(buf);
  const includeSeries = options.includeSeries ?? true;
  const n = header.recordCount;
  let series: DatSeries | undefined;
  if (includeSeries) {
    const totalLeak = new Float32Array(n);
    const excessLeak = new Float32Array(n);
    series = {
      hz: 1,
      pressure: new Float32Array(n),
      totalLeak,
      leak: totalLeak,
      excessLeak,
      leakFlow: excessLeak,
      snore: new Float32Array(n),
      flowAmplitude: new Float32Array(n),
    };
  }

  const pressureHist = new Uint32Array(2048); // tenths of cmH2O
  const leakHist = new Uint32Array(65536);
  const leakFlowHist = new Uint32Array(20000); // tenths of L/min, 0-2000
  let lfMax = 0;
  let pressureSum = 0;
  let valid = 0;
  let invalid = 0;
  let first: LocalTime | null = null;
  let last: LocalTime | null = null;
  const sessions: ActiveInterval[] = [];
  let intervalStart: LocalTime | null = null;
  const closeInterval = (lastTime: LocalTime) => {
    if (!intervalStart) return;
    const end = addSeconds(lastTime, 1);
    sessions.push({ start: intervalStart, end, minutes: (end.epochSec - intervalStart.epochSec) / 60 });
  };
  let pMin = Infinity;
  let pMax = -Infinity;
  let lMax = 0;

  for (const rec of iterateDatRecords(buf)) {
    if (series) {
      series.pressure[rec.index] = rec.setPressure;
      series.totalLeak[rec.index] = rec.totalLeak;
      series.excessLeak[rec.index] = rec.excessLeak;
      series.snore[rec.index] = rec.snore;
      series.flowAmplitude[rec.index] = rec.flowAmplitude;
    }
    if (!rec.valid) {
      invalid++;
      continue;
    }
    valid++;
    if (!first) first = rec.time;
    if (last && rec.time!.epochSec - last.epochSec > PAUSE_GAP_SECONDS) {
      closeInterval(last);
      intervalStart = rec.time;
    } else if (!intervalStart) intervalStart = rec.time;
    last = rec.time;
    const tenths = Math.round(rec.setPressure * 10);
    pressureHist[Math.min(tenths, pressureHist.length - 1)]!++;
    pressureSum += rec.setPressure;
    if (rec.setPressure < pMin) pMin = rec.setPressure;
    if (rec.setPressure > pMax) pMax = rec.setPressure;
    leakHist[rec.totalLeak]!++;
    if (rec.totalLeak > lMax) lMax = rec.totalLeak;
    const lfTenths = Math.min(Math.max(0, Math.round(rec.excessLeak * 10)), leakFlowHist.length - 1);
    leakFlowHist[lfTenths]!++;
    if (rec.excessLeak > lfMax) lfMax = rec.excessLeak;
  }

  if (last) closeInterval(last);
  const end = last ? addSeconds(last, 1) : null;
  const spanSeconds = sessions.reduce((sum, x) => sum + (x.end.epochSec - x.start.epochSec), 0);
  const totalLeak = {
    median: histPercentile(leakHist, valid, 0.5),
    p95: histPercentile(leakHist, valid, 0.95),
    max: lMax,
  };
  const excessLeak = {
    median: histPercentile(leakFlowHist, valid, 0.5) / 10,
    p95: histPercentile(leakFlowHist, valid, 0.95) / 10,
    max: round1(lfMax),
  };
  return {
    serial: header.serial,
    start: first,
    end,
    recordCount: n,
    invalidRecords: invalid,
    recordSeconds: valid,
    usageSeconds: spanSeconds,
    usageMinutes: spanSeconds / 60,
    sessions,
    pressure: {
      min: valid ? round1(pMin) : 0,
      mean: valid ? round3(pressureSum / valid) : 0,
      p90: histPercentile(pressureHist, valid, 0.9) / 10,
      p95: histPercentile(pressureHist, valid, 0.95) / 10,
      max: valid ? round1(pMax) : 0,
    },
    totalLeak,
    leak: { ...totalLeak },
    excessLeak,
    leakFlow: { ...excessLeak },
    ...(series ? { series } : {}),
  };
}

function addSeconds(t: LocalTime, s: number): LocalTime {
  const d = new Date((t.epochSec + s) * 1000);
  const p = (x: number, w = 2) => String(x).padStart(w, '0');
  const year = d.getUTCFullYear();
  const month = d.getUTCMonth() + 1;
  const day = d.getUTCDate();
  const hour = d.getUTCHours();
  const minute = d.getUTCMinutes();
  const second = d.getUTCSeconds();
  return {
    iso: `${p(year, 4)}-${p(month)}-${p(day)}T${p(hour)}:${p(minute)}:${p(second)}`,
    epochSec: t.epochSec + s,
    year,
    month,
    day,
    hour,
    minute,
    second,
  };
}

const round1 = (x: number) => Math.round(x * 10) / 10;
const round3 = (x: number) => Math.round(x * 1000) / 1000;
