/**
 * Types for the AS2X-series parser. Field names carry the confidence from
 * docs/FORMAT_NOTES.md where it matters. "Confirmed (NeoRead)" means Aeonmed's
 * own NeoRead software decodes the field this way. Fields marked Provisional
 * are Likely or Guess and may change.
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

/** Therapy mode byte (settings 0x02), per NeoRead. 3 and 4 are bilevel (BF models) only. */
export type TherapyModeLabel = 'CPAP' | 'Auto CPAP' | 'AUTOS' | 'S' | 'ST' | 'unknown';

/**
 * A `C5 C5` therapy-parameter block. Offsets are relative to the block start
 * and decoded as NeoRead's ReadTheropyParams does (Confirmed (NeoRead)).
 */
export interface SettingsSnapshot {
  /** Snapshot time (offset 0x21). Null if the block carries no valid time. NeoRead skips these bytes. */
  time: LocalTime | null;
  /** Raw u16 values at offsets 3, 5, 7, 9, in the card's own units (pressure x10). */
  pressureFields: number[];
  /** Byte 0x02: 0 CPAP, 1 Auto CPAP, 2 AUTOS, 3 S, 4 ST. */
  mode: number;
  modeLabel: TherapyModeLabel;
  /** i16 at 0x03 x0.1, cmH2O. NeoRead "StartPressure". */
  startPressure: number;
  /** i16 at 0x05 x0.1, cmH2O. NeoRead "TreatPressure" (fixed CPAP-mode pressure). */
  fixedPressure: number;
  /** i16 at 0x07 x0.1, cmH2O. NeoRead "MinTreatPressure" (the setting). */
  minPressure: number;
  /** i16 at 0x09 x0.1, cmH2O. NeoRead "MaxTreatPressure". */
  maxPressure: number;
  /** Byte 0x14: comfort (AS-Elex, NeoRead "Reslex") time. */
  comfortTime: number;
  /** Byte 0x15 & 0x0F: comfort (AS-Elex) level. */
  comfortLevel: number;
  /** Byte 0x16: leak alert, 0 = off. */
  leakAlert: number;
  /** Byte 0x17: humidifier level. */
  humidifierLevel: number;
  /** Byte 0x18: ramp time, minutes. */
  rampMinutes: number;
  /** Byte 0x19 != 0. */
  autoOn: boolean;
  /** Byte 0x1A != 0. */
  autoOff: boolean;
  /** Byte 0x1D: pressure unit (cmH2O or hPa). The value-to-unit mapping is Unresolved, so it stays raw. */
  pressureUnit: number;
  /** The full 128-byte block, copied. */
  raw: Uint8Array;
}

export interface DatHeader {
  /** File-created time at offset 0 (Likely). */
  created: LocalTime | null;
  /**
   * Header byte 8, the record format version (Confirmed (NeoRead)): 1 = 512-byte
   * `<<` records, 2 = 812-byte `MM` records. Only version 2 records are decoded;
   * any value other than 1 is read with the version 2 layout.
   */
  formatVersion: number;
  /** False for version 1 files: `iterateDatRecords` then yields nothing. */
  recordsSupported: boolean;
  /** The first four characters of the device ID string at 0x50, for example `AS2X` (Confirmed). */
  deviceModel: string;
  /** NeoRead's model name for the first three ID characters (for example AS2 -> AS100A), or null. */
  modelName: string | null;
  serial: string;
  /** Byte 0x68 (NeoRead DeviceType), raw. */
  deviceType: number;
  /** Software (firmware) version at 0x69, printed as NeoRead does (for example `1.01.35`). Null if unset. */
  softwareVersion: string | null;
  /** Hardware version at 0x6D, same format. Null if unset. */
  hardwareVersion: string | null;
  settings: SettingsSnapshot | null;
  /** Whole records in the file (812 bytes each; 512 for version 1). */
  recordCount: number;
  /** Bytes after the last whole record (0 for a healthy file). */
  trailingBytes: number;
}

export interface DatWaveforms {
  /** 50 Hz set (target) pressure, slots 20-69, cmH2O. Confirmed. */
  setPressure: Float32Array;
  /**
   * 50 Hz slots 70-119, cmH2O. NeoRead's "PressureFdb" (pressure feedback):
   * the channel NeoRead plots as pressure (Confirmed (NeoRead)). Blower side (Likely).
   */
  pressureFeedback: Float32Array;
  /** 50 Hz measured pressure, mask side, slots 120-169, cmH2O. Likely. */
  maskPressure: Float32Array;
  /** 50 Hz snore, slots 170-219, raw (NeoRead graph range 0-200). Confirmed (NeoRead). */
  snore: Float32Array;
  /** 50 Hz patient flow, slots 220-269, L/min (value - 500). Confirmed (NeoRead "Flow"). */
  patientFlow: Float32Array;
  /** 50 Hz leak flow, slots 270-319, L/min as stored (NeoRead graph range 0-50). Confirmed (NeoRead). */
  leakFlow: Float32Array;
  /** 50 Hz total flow, slots 320-369, L/min (value - 500). Likely; NeoRead skips this channel. */
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
  /** Slot 384, L/min (Likely). Which leak figure matches the machine's own is unverified. */
  leak: number;
  /** Mean of slots 270-319 over the second, L/min. NeoRead's leak flow. */
  leakFlow: number;
  /** Mean of slots 170-219 over the second, raw snore units. */
  snore: number;
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
  /** Per-second leak flow (slots 270-319, NeoRead's leak flow), L/min, nearest-rank on 0.1 steps. */
  leakFlow: { median: number; p95: number; max: number };
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
  leakFlow: Float32Array;
  snore: Float32Array;
  flowAmplitude: Float32Array;
}

/**
 * One A5A5 index item with its B5B5 statistics. Offsets are relative to the
 * A5A5 block start and decoded as NeoRead does for firmware 01.01.19 and later
 * (older firmware uses other scales and is not supported).
 */
export interface SessionSummary {
  sessionNumber: number;
  start: LocalTime | null;
  /** 0x54, minutes (NeoRead "TreatMinutes"). */
  usageMinutes: number;
  /** i16 at 0x4C x0.1, cmH2O. NeoRead "InitPressure" (start pressure). */
  startPressure: number;
  /**
   * cmH2O. `mean` 0x4E and `p90` 0x50, x0.1 (NeoRead AvgeTreatPressure, P90).
   * `min` is a deprecated alias of `startPressure` (0x4C is the start pressure, not a minimum).
   */
  pressure: { /** @deprecated use `startPressure` */ min: number; mean: number; p90: number };
  /** 0x5E x0.1 (NeoRead "MaxP"). Likely the observed maximum target pressure, NOT the configured maximum. */
  observedMaxPressure: number;
  /** 0x52 x0.1: minutes with leak above 90 L/min (NeoRead "LeakMinutes (>90 LPM)"). */
  largeLeakMinutes: number;
  /** 0x56 x0.1: snore index (NeoRead "SNI"). */
  snoreIndex: number;
  /**
   * Events per hour (raw x0.1). Confirmed, each = round-half-up(count * 600 / usageMinutes) / 10:
   * ahi (0x58, E1+E2+E3), apneaIndex (0x5A, E1+E2), centralApneaIndex (0x5C, E2, NeoRead "CAI"),
   * hypopneaIndex (0x60, E3). 0xFFFF in 0x5A-0x60 reads as 0, as in NeoRead.
   */
  ahi: number;
  apneaIndex: number;
  centralApneaIndex: number;
  /** @deprecated alias of `centralApneaIndex`. */
  apnea2Index: number;
  /**
   * apneaIndex - centralApneaIndex, rounded to 0.1, never below 0. Derived:
   * both inputs are rounded separately, so it can differ by 0.1 from the E1 count's own index.
   */
  obstructiveApneaIndex: number;
  hypopneaIndex: number;
  /** True when the `B5 B5` marker at 0x40 is present. */
  hasB5Marker: boolean;
}

/**
 * Event codes. NeoRead uses 0xA0-0xAF; the sample card's newer firmware uses
 * the same order at 0xE0-0xEF.
 * - Confirmed (NeoRead): A0-A7, AE, AF; E1 obstructive and E2 central (E2
 *   matches NeoRead's CAI on every night), E3 hypopnea.
 * - Likely (same order + 0x40): E0, E4-E7, EE, EF.
 * - Unresolved: E8 ('level', no NeoRead equivalent; likely a leak level).
 * Other codes in A0-AF / E0-EF decode as 'unknown'.
 */
export type TherapyEventType =
  | 'pressureChange' // A0 / E0
  | 'obstructiveApnea' // A1 / E1
  | 'centralApnea' // A2 / E2
  | 'hypopnea' // A3 / E3
  | 'snoreLevel' // A4 / E4
  | 'largeLeak' // A5 / E5
  | 'flowLimitation' // A6 / E6
  | 'snoreCount' // A7 / E7
  | 'level' // E8, Unresolved (likely leak level)
  | 'treatmentStart' // AE / EE
  | 'treatmentEnd' // AF / EF
  | 'unknown';

export interface TherapyEvent {
  /** Stamped at the END of the event (Confirmed for E1-E3). */
  time: LocalTime;
  eventType: TherapyEventType;
  /** The raw code byte, for example 0xE1. */
  code: number;
  /** The raw value byte. For E8 ('level') it is a coarse score, 0-14 observed. */
  value: number;
  /** For pressureChange, apneas and hypopnea: value x 0.5 cmH2O (Likely for E0, Confirmed for E3). */
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
