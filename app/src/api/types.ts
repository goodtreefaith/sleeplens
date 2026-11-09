// Types from docs/APP_CONTRACT.md. Keep in step with the Rust backend (serde camelCase).

export type Source = "machine" | "computed";
export type PressureUnit = "hPa" | "cmH2O" | "unknown";

export interface AppInfo {
  version: string;
  needsReread: number;
  dataDir: string;
  dbSizeBytes: number;
  device: Device | null;
  nightCount: number;
  firstDate: string | null;
  lastDate: string | null;
  lastImportAt: string | null;
}

export interface Device {
  brand: "Aeonmed";
  model: string;
  family: "AS2X";
  firmware: string | null;
  hardware: string | null;
  serialMasked: string;
  pressureUnit: PressureUnit;
}

export interface CardCandidate {
  path: string;
  label: string;
  family: "AS2X";
  nights: number;
  months: string[];
}

export interface ImportStarted {
  importId: string;
}

export interface ImportResult {
  importId: string;
  status: "ok" | "cancelled" | "error";
  nightsAdded: number;
  nightsUpdated: number;
  nightsSkipped: number;
  nightsFailed: { date: string; reason: string }[];
  message?: string;
}

export type ImportPhase = "scanning" | "summaries" | "nights" | "done" | "cancelled" | "error";

export interface ImportProgress {
  phase: ImportPhase;
  done: number;
  total: number;
  currentDate?: string;
  message?: string;
}

export interface EventCounts {
  obstructive: number;
  central: number;
  hypopnea: number;
}

export interface NightListItem {
  date: string;
  start: string | null;
  end: string | null;
  usageMinutes: number;
  usageSource: Source;
  ahi: number | null;
  pressureMean: number | null;
  pressureP90: number | null;
  pressureSource: Source;
  excessLeakMedian: number | null;
  excessLeakP95: number | null;
  sleepScore: number | null;
  interruptions: number;
  largeLeakMinutes: number | null;
  eventCounts: EventCounts;
  settingsChanged: boolean;
}

export interface NightSummary {
  startPressure: number | null;
  maxPressure: number | null;
  ai: number | null;
  cai: number | null;
  oai: number | null;
  hi: number | null;
  sni: number | null;
}

export interface NightSeries {
  seriesStart: string;
  pressure: (number | null)[];
  excessLeak: (number | null)[];
  totalLeak: (number | null)[];
  flowAmplitude: (number | null)[];
  snore: (number | null)[];
}

export interface Session {
  start: string;
  end: string;
  minutes: number;
}

export interface NightDetail extends NightListItem {
  score: SleepScore | null;
  stages: SleepStageEstimate | null;
  tips: Tip[];
  sessions: Session[];
  summary: NightSummary;
  settings: Settings | null;
  events: TherapyEvent[];
  series: NightSeries | null;
}

export type EventKind =
  | "obstructiveApnea"
  | "centralApnea"
  | "hypopnea"
  | "pressureChange"
  | "leakLevel"
  | "snore"
  | "largeLeak"
  | "flowLimitation"
  | "other";

export interface TherapyEvent {
  time: string;
  kind: EventKind;
  code: number;
  value: number;
  pressure: number | null;
}

export type ModeLabel = "CPAP" | "Auto CPAP" | "AUTOS" | "S" | "ST" | "unknown";

export interface Settings {
  mode: number;
  modeLabel: ModeLabel;
  startPressure: number;
  minPressure: number;
  maxPressure: number;
  fixedPressure: number;
  rampMinutes: number;
  humidifierLevel: number;
  comfortLevel: number;
  autoOn: boolean;
  autoOff: boolean;
  leakAlert: boolean;
  pressureUnit: PressureUnit;
}

export interface SettingsChange {
  date: string;
  time: string | null;
  settings: Settings;
  changed: (keyof Settings)[];
}

export interface TrendPoint {
  sleepScore: number | null;
  excessLeakP95: number | null;
  prescriptionChanged: boolean;
  date: string;
  usageMinutes: number;
  ahi: number | null;
  pressureP90: number | null;
  excessLeakMedian: number | null;
  settingsChanged: boolean;
}

export interface Trends {
  from: string;
  to: string;
  nights: number;
  daysInRange: number;
  validNights: number;
  compliancePct: number;
  avgUsageMinutes: number;
  avgAhi: number | null;
  avgPressureP90: number | null;
  avgExcessLeak: number | null;
  avgSleepScore: number | null;
  avgExcessLeakP95: number | null;
  points: TrendPoint[];
}

export interface DateRange {
  from?: string;
  to?: string;
}

export type Unlisten = () => void;

export type DragDropEvent =
  | { type: "enter" | "over"; paths: string[] }
  | { type: "drop"; paths: string[] }
  | { type: "leave" };

/** One interface, two implementations (Tauri IPC and in-memory mock). */
export interface SleepLensApi {
  readonly kind: "tauri" | "mock";
  appInfo(): Promise<AppInfo>;
  findCards(): Promise<CardCandidate[]>;
  importCard(path: string): Promise<ImportStarted>;
  cancelImport(): Promise<null>;
  listNights(range?: DateRange): Promise<NightListItem[]>;
  nightDetail(date: string): Promise<NightDetail>;
  trends(range?: DateRange): Promise<Trends>;
  periodSummary(period: Period, end?: string): Promise<PeriodSummary>;
  settingsHistory(): Promise<SettingsChange[]>;
  deleteAllData(): Promise<null>;
  loadSampleData(): Promise<ImportStarted>;

  onImportProgress(handler: (p: ImportProgress) => void): Promise<Unlisten>;
  onImportFinished(handler: (r: ImportResult) => void): Promise<Unlisten>;

  // Platform helpers (not IPC commands): native folder picker and window drag-and-drop.
  /** Opens the native folder picker. Resolves to the chosen path, or null if cancelled. */
  pickFolder(): Promise<string | null>;
  /**
   * Window-level drag and drop of folders. In Tauri this gives real file system paths.
   * Returns null when the platform cannot supply paths (the UI then uses HTML drag events).
   */
  onDragDrop(handler: (e: DragDropEvent) => void): Promise<Unlisten | null>;
}

export type Period = "week" | "month" | "6months" | "year";
export type SleepStage = "wake" | "light" | "deep" | "rem" | "noData";
export interface StageMinutes { wake: number; light: number; deep: number; rem: number }
export interface SleepStageEstimate {
  start: string;
  epochSeconds: number;
  stages: SleepStage[];
  minutes: StageMinutes;
  signalQuality: number;
  estimatorVersion: number;
}
export interface SleepScore {
  total: number;
  label: "Great" | "Good" | "Fair" | "Poor";
  estimated: true;
  components: {
    key: "duration" | "breathing" | "maskSeal" | "continuity" | "sleepStages";
    label: string;
    points: number;
    maxPoints: number;
    detail: string;
  }[];
}
export interface Tip {
  id: string;
  severity: "good" | "info" | "watch" | "talk";
  title: string;
  body: string;
  learnSlug: string | null;
  evidence: string | null;
  evidenceDates: string[];
}
export interface PeriodSummary {
  headline: string;
  clampedToFirstNight: boolean;
  period: Period;
  from: string; to: string; daysInRange: number;
  nights: number; validNights: number; compliancePct: number;
  avgUsageMinutes: number | null;
  avgAhi: number | null;
  avgSleepScore: number | null;
  avgExcessLeakP95: number | null;
  avgPressureP90: number | null;
  avgStageMinutes: StageMinutes | null;
  previous: {
    avgUsageMinutes: number | null;
    avgAhi: number | null;
    avgSleepScore: number | null;
    compliancePct: number | null;
  };
  bestNight: { date: string; sleepScore: number } | null;
  worstNight: { date: string; sleepScore: number } | null;
  scoreSeries: { date: string; sleepScore: number | null; usageMinutes: number }[];
  tips: Tip[];
}
