/**
 * Types for the AS2X-series parser. Field names carry the confidence from
 * docs/FORMAT_NOTES.md where it matters (for example `apnea1`, not
 * "obstructive": the E1/E2 split is unconfirmed). Fields marked Provisional
 * are Likely or Guess in FINDINGS and may change.
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
   * Raw u16 values at offsets 3, 5, 7, 9, in the card's own units (pressure x10).
   * Offset 5 is a Guess (fixed-mode pressure) and stays raw only.
   */
  pressureFields: number[];
  /** u16 at 0x03 / 10, cmH2O. Provisional (Likely): minimum pressure. */
  minPressure: number;
  /** u16 at 0x07 / 10, cmH2O. Provisional (Likely): start pressure. Always equals minPressure in the sample. */
  startPressure: number;
  /** u16 at 0x09 / 10, cmH2O. Provisional (Likely): maximum pressure setting. */
  maxPressure: number;
  /** Byte 0x17, 1-5. Confirmed humidifier level. */
  humidifierLevel: number;
  /**
   * Other bytes stay raw. Provisional guesses: 0x15 u16 comfort level, 0x18 u8
   * ramp minutes, 0x1F u8 enum (mask type?). Everything else is Unresolved.
   */
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
  /** Slot 381, event-related measurement, 0-150 (Confirmed association with the E1/E2 split; meaning Unresolved). */
  stateCode: number;
  /** Only when `iterateDatRecords(buf, { waveforms: true })`. */
  waveforms?: DatWaveforms;
}

export interface DatSummary {
  serial: string;
  start: LocalTime | null;
  /** Timestamp of the last record plus one second (wall clock, includes pauses). */
  end: LocalTime | null;
  recordCount: number;
  invalidRecords: number;
  /** Valid records. Timestamps skip a second about every 341 s, so this is a little under the wall-clock span. */
  recordSeconds: number;
  /** Sum of the active intervals in `sessions`, in seconds (pauses excluded). */
  usageSeconds: number;
  /** usageSeconds / 60, unrounded. The device's own figure rounds up. */
  usageMinutes: number;
  /** Active intervals: the record stream split where the timestamp gap exceeds 60 s. */
  sessions: ActiveInterval[];
  /** Per-second set pressure statistics, cmH2O. Percentiles are nearest-rank. */
  pressure: { min: number; mean: number; p90: number; p95: number; max: number };
  /** Slot 384, L/min. Median is the nearest-rank (lower) median. */
  leak: { median: number; p95: number; max: number };
  /** One value per second per channel; absent when `includeSeries: false`. */
  series?: DatSeries;
}

/** One uninterrupted stretch of therapy inside a night file. */
export interface ActiveInterval {
  start: LocalTime;
  /** Last record's timestamp plus one second. */
  end: LocalTime;
  /** (end - start) / 60, unrounded. 1-second clock-drift skips count as continuous. */
  minutes: number;
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
  /**
   * cmH2O. `p90` is the coarse (0.5 steps) P90 target pressure at 0x50 (Likely;
   * the exact rule is Unresolved and not a plain percentile).
   */
  pressure: { min: number; mean: number; p90: number };
  /** 0x5E / 10. Likely the observed maximum target pressure (NOT the configured maximum). */
  observedMaxPressure: number;
  /**
   * Events per hour (raw / 10). Confirmed, each = round-half-up(count * 600 / usageMinutes) / 10:
   * ahi (0x58, E1+E2+E3), apneaIndex (0x5A, E1+E2), apnea2Index (0x5C, E2 only), hypopneaIndex (0x60, E3).
   */
  ahi: number;
  apneaIndex: number;
  apnea2Index: number;
  hypopneaIndex: number;
  /** True when the `B5 B5` marker at 0x40 is present. */
  hasB5Marker: boolean;
}

export type TherapyEventType =
  | 'pressureChange'
  | 'apnea1' // E1, provisional: likely obstructive
  | 'apnea2' // E2, provisional: likely central
  | 'hypopnea'
  | 'level'
  | 'sessionMarker'
  | 'unknown';

export interface TherapyEvent {
  /** Stamped at the END of the event (Confirmed for E1-E3). */
  time: LocalTime;
  eventType: TherapyEventType;
  /**
   * Provisional (Likely, not proven by the card alone): E1 'obstructive', E2
   * 'central'. Keep `code` / `eventType` as the source of truth.
   */
  likelyKind?: 'obstructive' | 'central';
  /** The raw code byte, for example 0xE1. */
  code: number;
  /** The raw value byte. For E8 ('level') it is a coarse score, 0-14 observed. */
  value: number;
  /** For pressureChange and E1-E3: value x 0.5 cmH2O (Likely for E0, Confirmed for E3). */
  pressure?: number;
}

export interface TextEvent {
  time: LocalTime;
  text: string;
}

/**
 * One 64-byte counter record (block type at 0x4000-0x5B7F, two per block).
 * Counter A is Likely operating time since filter reset (unit ~61.44 s, a
 * Guess); B cumulative; C is always B - 1. Provisional.
 */
export interface CounterRecord {
  /** `YYYY-MM-DD`. A date, not a timestamp. */
  date: string;
  year: number;
  month: number;
  day: number;
  counterA: number;
  counterB: number;
  counterC: number;
}

export interface BkpStats {
  blocks: number;
  emptyBlocks: number;
  unknownBlocks: number;
  counterBlocks: number;
  /** Therapy-block entries that were skipped as empty or invalid. */
  skippedEntries: number;
}

export interface BkpParseResult {
  settings: SettingsSnapshot[];
  sessions: SessionSummary[];
  /** Sorted by time (the log is a ring buffer, so file order is not chronological). */
  events: TherapyEvent[];
  textEvents: TextEvent[];
  /** Counter records in file order (ring buffer), then sorted by date. */
  counters: CounterRecord[];
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
