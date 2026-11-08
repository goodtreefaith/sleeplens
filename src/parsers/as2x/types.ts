/**
 * Types for the AS2X-series parser. Field names carry the confidence from
 * docs/FORMAT_NOTES.md where it matters (for example `apnea1`, not
 * "obstructive": the E1/E2 split is unconfirmed).
 */

/** A device-local wall-clock time. The card stores no time zone. */
export interface LocalTime {
  /** `YYYY-MM-DDTHH:MM:SS`, no zone suffix. */
  iso: string;
  /** Seconds since 1970-01-01 treating the wall clock as UTC. Only for ordering and differences. */
  epochSec: number;
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
}

export interface SettingsSnapshot {
  /** Snapshot time (offset 0x21). Null if the block carries no valid time. */
  time: LocalTime | null;
  /**
   * u16 values found at offsets 3, 5, 7, 9, in the card's own units. Believed
   * to be pressures x10 (min / ? / ? / max). Field map is still open (Guess).
   */
  pressureFields: number[];
  /** Byte near offset 0x2E. Believed to be the humidifier level (Guess). */
  humidifierRaw: number;
  /** The full 128-byte block, copied. */
  raw: Uint8Array;
}

export interface DatHeader {
  /** File-created time at offset 0 (Likely). */
  created: LocalTime | null;
  /** From the `AS2X<SERIAL>` string at 0x50 (Confirmed). */
  deviceModel: string;
  serial: string;
  settings: SettingsSnapshot | null;
  /** Whole 812-byte records in the file. */
  recordCount: number;
  /** Bytes after the last whole record (0 for a healthy file). */
  trailingBytes: number;
}

export interface DatWaveforms {
  /** 50 Hz set (target) pressure, cmH2O. Confirmed. */
  setPressure: Float32Array;
  /** 50 Hz measured pressure, blower side, cmH2O. Likely. */
  blowerPressure: Float32Array;
  /** 50 Hz measured pressure, mask side, cmH2O. Likely. */
  maskPressure: Float32Array;
  /** 50 Hz patient flow, L/min (value - 500). Likely. */
  patientFlow: Float32Array;
  /** 50 Hz total flow, L/min (value - 500). Likely. */
  totalFlow: Float32Array;
}

/** One second of therapy (one 812-byte record). */
export interface DatRecord {
  index: number;
  /** Null if the record's magic or timestamp is invalid. */
  time: LocalTime | null;
  /** Monday = 0. */
  weekday: number;
  valid: boolean;
  /** Mean set pressure over the second, cmH2O. */
  setPressure: number;
  /** Mean mask-side measured pressure over the second, cmH2O (Likely). */
  maskPressure: number;
  /** Max minus min of patient flow within the second, L/min (Likely). */
  flowAmplitude: number;
  /** Slot 384, L/min (Likely). */
  leak: number;
  /** Slot 379, elapsed seconds counter (Likely). */
  elapsedCounter: number;
  /** Slot 381, small algorithm state code (Guess). */
  stateCode: number;
  /** Only when `iterateDatRecords(buf, { waveforms: true })`. */
  waveforms?: DatWaveforms;
}

export interface DatSummary {
  serial: string;
  start: LocalTime | null;
  /** Timestamp of the last record plus one second. */
  end: LocalTime | null;
  recordCount: number;
  invalidRecords: number;
  /** Valid records. Timestamps skip a second about every 341 s, so this is a little under the wall-clock span. */
  recordSeconds: number;
  /** Wall-clock span from first record to end, in seconds. Matches the device's own usage figure (it rounds up). */
  usageSeconds: number;
  /** usageSeconds / 60, unrounded. */
  usageMinutes: number;
  /** Per-second set pressure statistics, cmH2O. Percentiles are nearest-rank. */
  pressure: { min: number; mean: number; p95: number; max: number };
  /** Slot 384, L/min. Median is the nearest-rank (lower) median. */
  leak: { median: number; p95: number; max: number };
  /** One value per second per channel; absent when `includeSeries: false`. */
  series?: DatSeries;
}

export interface DatSeries {
  hz: 1;
  pressure: Float32Array;
  leak: Float32Array;
  flowAmplitude: Float32Array;
}

export interface SessionSummary {
  sessionNumber: number;
  start: LocalTime | null;
  usageMinutes: number;
  /** cmH2O. */
  pressure: { min: number; mean: number; p95: number };
  /** "Max pressure setting x10" at 0x5E (Likely), cmH2O. */
  maxPressureSetting: number;
  /** Events per hour (raw / 10). AHI is Confirmed; AI and HI are Likely. */
  ahi: number;
  apneaIndex: number;
  hypopneaIndex: number;
  /** True when the `B5 B5` marker at 0x40 is present. */
  hasB5Marker: boolean;
}

export type TherapyEventType =
  | 'pressureChange'
  | 'apnea1'
  | 'apnea2'
  | 'hypopnea'
  | 'level'
  | 'sessionMarker'
  | 'unknown';

export interface TherapyEvent {
  /** Stamped at the END of the event (Confirmed for E1-E3). */
  time: LocalTime;
  eventType: TherapyEventType;
  /** The raw code byte, for example 0xE1. */
  code: number;
  /** The raw value byte. */
  value: number;
  /** For pressureChange and E1-E3: value x 0.5 cmH2O (Likely for E0, Confirmed for E3). */
  pressure?: number;
}

export interface TextEvent {
  time: LocalTime;
  text: string;
}

export interface BkpStats {
  blocks: number;
  emptyBlocks: number;
  unknownBlocks: number;
  /** Therapy-block entries that were skipped as empty or invalid. */
  skippedEntries: number;
}

export interface BkpParseResult {
  settings: SettingsSnapshot[];
  sessions: SessionSummary[];
  /** Sorted by time (the log is a ring buffer, so file order is not chronological). */
  events: TherapyEvent[];
  textEvents: TextEvent[];
  stats: BkpStats;
}

export interface Nightfile {
  /** `YYYYMMDD`, the evening the night started. */
  date: string;
  /** Path exactly as given to `detectAs2x`. */
  path: string;
  /** The month folder name. */
  folder: string;
}

export interface As2xDetection {
  detected: boolean;
  serial: string | null;
  nights: Nightfile[];
  /** Path of the `<serial>.bkp` file, if present. */
  bkp: string | null;
  /** Sorted `YYYYMM` strings seen in folder names. */
  months: string[];
}
