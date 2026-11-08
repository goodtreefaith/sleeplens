import type {
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
export const DAT_RECORD_SIZE = 812;
export const SAMPLES_PER_RECORD = 400;
export const WAVE_SAMPLES = 50;

/** Slot indexes into a record's 400 u16 values (see FORMAT_NOTES). */
export const SLOT = {
  setPressure: 20,
  blowerPressure: 70,
  maskPressure: 120,
  patientFlow: 220,
  totalFlow: 320,
  elapsed: 379,
  state: 381,
  leak: 384,
} as const;

const RECORD_DATA_OFFSET = 12;
const FLOW_OFFSET = 500;

export function parseDatHeader(buf: Uint8Array): DatHeader {
  const dv = makeDataView(buf);
  const bodyBytes = Math.max(0, buf.length - DAT_HEADER_SIZE);
  const id = buf.length >= 0x60 ? readAscii(buf, 0x50, 16) : '';
  const model = id.slice(0, 4);
  return {
    created: readTimestamp(dv, 0),
    deviceModel: model,
    serial: id.slice(4),
    settings: parseSettingsBlock(buf, 0x140),
    recordCount: Math.floor(bodyBytes / DAT_RECORD_SIZE),
    trailingBytes: bodyBytes % DAT_RECORD_SIZE,
  };
}

export interface IterateOptions {
  /** Also decode the 50 Hz waveforms of each record (allocates per record). Default false. */
  waveforms?: boolean;
}

/**
 * Yield one record per second of therapy. Records are decoded lazily and never
 * retained, so a 30 MB night is processed with constant memory.
 */
export function* iterateDatRecords(
  buf: Uint8Array,
  options: IterateOptions = {},
): Generator<DatRecord> {
  const dv = makeDataView(buf);
  const count = parseDatHeader(buf).recordCount;
  for (let i = 0; i < count; i++) {
    const base = DAT_HEADER_SIZE + i * DAT_RECORD_SIZE;
    const magicOk = buf[base] === 0x4d && buf[base + 1] === 0x4d;
    const time = magicOk ? readTimestamp(dv, base + 4) : null;
    const data = base + RECORD_DATA_OFFSET;
    const u16 = (slot: number) => dv.getUint16(data + slot * 2, true);

    let setSum = 0;
    let maskSum = 0;
    let flowMin = 65535;
    let flowMax = 0;
    for (let k = 0; k < WAVE_SAMPLES; k++) {
      setSum += u16(SLOT.setPressure + k);
      maskSum += u16(SLOT.maskPressure + k);
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
      leak: u16(SLOT.leak),
      elapsedCounter: u16(SLOT.elapsed),
      stateCode: u16(SLOT.state),
    };
    if (options.waveforms) rec.waveforms = readWaveforms(dv, data);
    yield rec;
  }
}

function readWaveforms(dv: DataView, data: number): DatWaveforms {
  const read = (slot: number, scale: (v: number) => number) => {
    const out = new Float32Array(WAVE_SAMPLES);
    for (let k = 0; k < WAVE_SAMPLES; k++) out[k] = scale(dv.getUint16(data + (slot + k) * 2, true));
    return out;
  };
  const pressure = (v: number) => v / 10;
  const flow = (v: number) => v - FLOW_OFFSET;
  return {
    setPressure: read(SLOT.setPressure, pressure),
    blowerPressure: read(SLOT.blowerPressure, pressure),
    maskPressure: read(SLOT.maskPressure, pressure),
    patientFlow: read(SLOT.patientFlow, flow),
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

export interface SummarizeOptions {
  /** Keep a 1 Hz series per channel. Default true. */
  includeSeries?: boolean;
}

/** One streaming pass over a night file: usage, pressure and leak statistics, 1 Hz series. */
export function summarizeDat(buf: Uint8Array, options: SummarizeOptions = {}): DatSummary {
  const header = parseDatHeader(buf);
  const includeSeries = options.includeSeries ?? true;
  const n = header.recordCount;
  const series: DatSeries | undefined = includeSeries
    ? {
        hz: 1,
        pressure: new Float32Array(n),
        leak: new Float32Array(n),
        flowAmplitude: new Float32Array(n),
      }
    : undefined;

  const pressureHist = new Uint32Array(2048); // tenths of cmH2O
  const leakHist = new Uint32Array(65536);
  let pressureSum = 0;
  let valid = 0;
  let invalid = 0;
  let first: LocalTime | null = null;
  let last: LocalTime | null = null;
  let pMin = Infinity;
  let pMax = -Infinity;
  let lMax = 0;

  for (const rec of iterateDatRecords(buf)) {
    if (series) {
      series.pressure[rec.index] = rec.setPressure;
      series.leak[rec.index] = rec.leak;
      series.flowAmplitude[rec.index] = rec.flowAmplitude;
    }
    if (!rec.valid) {
      invalid++;
      continue;
    }
    valid++;
    if (!first) first = rec.time;
    last = rec.time;
    const tenths = Math.round(rec.setPressure * 10);
    pressureHist[Math.min(tenths, pressureHist.length - 1)]!++;
    pressureSum += rec.setPressure;
    if (rec.setPressure < pMin) pMin = rec.setPressure;
    if (rec.setPressure > pMax) pMax = rec.setPressure;
    leakHist[rec.leak]!++;
    if (rec.leak > lMax) lMax = rec.leak;
  }

  const end = last ? addSeconds(last, 1) : null;
  const spanSeconds = first && end ? end.epochSec - first.epochSec : 0;
  return {
    serial: header.serial,
    start: first,
    end,
    recordCount: n,
    invalidRecords: invalid,
    recordSeconds: valid,
    usageSeconds: spanSeconds,
    usageMinutes: spanSeconds / 60,
    pressure: {
      min: valid ? round1(pMin) : 0,
      mean: valid ? round3(pressureSum / valid) : 0,
      p95: histPercentile(pressureHist, valid, 0.95) / 10,
      max: valid ? round1(pMax) : 0,
    },
    leak: {
      median: histPercentile(leakHist, valid, 0.5),
      p95: histPercentile(leakHist, valid, 0.95),
      max: lMax,
    },
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
