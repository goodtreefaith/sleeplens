# SleepLens app contract

The interface between the Rust backend (`app/src-tauri`) and the React UI
(`app/src`). Both sides build against this file. JSON field names are
camelCase (`#[serde(rename_all = "camelCase")]`). Dates are `YYYY-MM-DD`
(the evening the night started); times are local wall-clock
`YYYY-MM-DDTHH:MM:SS` with no zone. Pressures are in the card's unit
(`pressureUnit`, `"hPa"` on the AS100); flows in L/min. `null` means "not
recorded".

## Commands (`invoke(name, args)`)

| Command | Args | Returns |
| --- | --- | --- |
| `app_info` | – | `AppInfo` |
| `find_cards` | – | `CardCandidate[]` (mounted volumes that look like a supported card) |
| `import_card` | `{ path: string }` | `ImportStarted` (import runs in the background) |
| `cancel_import` | – | `null` |
| `list_nights` | `{ from?: string, to?: string }` | `NightListItem[]` sorted by date ascending |
| `night_detail` | `{ date: string }` | `NightDetail` |
| `trends` | `{ from?: string, to?: string }` | `Trends` |
| `settings_history` | – | `SettingsChange[]` |
| `delete_all_data` | `{ confirm: true }` | `null` |
| `load_sample_data` | – | `ImportStarted` (imports the bundled synthetic card) |

## Events (`listen(name)`)

| Event | Payload |
| --- | --- |
| `import-progress` | `{ phase: "scanning" \| "summaries" \| "nights" \| "done" \| "cancelled" \| "error", done: number, total: number, currentDate?: string, message?: string }` |
| `import-finished` | `ImportResult` |

## Types

```ts
type Source = "machine" | "computed";        // machine = the device's own summary (matches its screen)

interface AppInfo {
  version: string;
  dataDir: string;                 // where the database lives
  dbSizeBytes: number;
  device: Device | null;           // null before the first import
  nightCount: number;
  firstDate: string | null;
  lastDate: string | null;
  lastImportAt: string | null;
}

interface Device {
  brand: "Aeonmed";
  model: string;                   // "AS100A"
  family: "AS2X";
  firmware: string | null;         // "1.01.47.01"
  hardware: string | null;
  serialMasked: string;            // never the full serial in the UI: "•••" + last 2
  pressureUnit: "hPa" | "cmH2O" | "unknown";
}

interface CardCandidate { path: string; label: string; family: "AS2X"; nights: number; months: string[] }

interface ImportStarted { importId: string }

interface ImportResult {
  importId: string;
  status: "ok" | "cancelled" | "error";
  nightsAdded: number;
  nightsUpdated: number;
  nightsSkipped: number;          // already imported and unchanged
  nightsFailed: { date: string; reason: string }[];
  message?: string;
}

interface NightListItem {
  date: string;
  start: string | null;            // first mask-on
  end: string | null;              // last mask-off
  usageMinutes: number;
  usageSource: Source;
  ahi: number | null;              // null when the .bkp no longer has this night
  pressureMean: number | null;
  pressureP90: number | null;
  pressureSource: Source;
  excessLeakMedian: number | null; // L/min, slots 270-319
  largeLeakMinutes: number | null; // machine "LEAK90"
  eventCounts: EventCounts;
  settingsChanged: boolean;        // settings differ from the previous night
}

interface EventCounts { obstructive: number; central: number; hypopnea: number }

interface NightDetail extends NightListItem {
  sessions: { start: string; end: string; minutes: number }[];
  summary: {                       // machine summary; all null if absent
    startPressure: number | null;
    maxPressure: number | null;    // highest pressure reached
    ai: number | null; cai: number | null; oai: number | null; hi: number | null;
    sni: number | null;
  };
  settings: Settings | null;
  events: TherapyEvent[];
  series: {                        // 1 Hz, aligned: series[i] is at seriesStart + i seconds
    seriesStart: string;           // gaps (mask off) are NaN in the JSON as null
    pressure: (number | null)[];
    excessLeak: (number | null)[];
    totalLeak: (number | null)[];
    flowAmplitude: (number | null)[];
    snore: (number | null)[];
  } | null;                        // null when the night file was not imported
}

interface TherapyEvent {
  time: string;
  kind: "obstructiveApnea" | "centralApnea" | "hypopnea" | "pressureChange" | "leakLevel" | "snore" | "largeLeak" | "flowLimitation" | "other";
  code: number;                    // raw code byte
  value: number;
  pressure: number | null;         // for pressureChange and apnea/hypopnea
}

interface Settings {
  mode: number; modeLabel: "CPAP" | "Auto CPAP" | "AUTOS" | "S" | "ST" | "unknown";
  startPressure: number; minPressure: number; maxPressure: number; fixedPressure: number;
  rampMinutes: number; humidifierLevel: number; comfortLevel: number;
  autoOn: boolean; autoOff: boolean; leakAlert: boolean;
  pressureUnit: "hPa" | "cmH2O" | "unknown";
}

interface SettingsChange { date: string; time: string | null; settings: Settings; changed: (keyof Settings)[] }

interface Trends {
  from: string; to: string;
  nights: number;                  // nights with any use
  daysInRange: number;
  validNights: number;             // usage > 240 min (machine rule)
  compliancePct: number;           // validNights / daysInRange * 100
  avgUsageMinutes: number;
  avgAhi: number | null;
  avgPressureP90: number | null;
  avgExcessLeak: number | null;
  points: { date: string; usageMinutes: number; ahi: number | null; pressureP90: number | null; excessLeakMedian: number | null; settingsChanged: boolean }[];
}
```

## Rules both sides follow

- Prefer machine summary values (`source: "machine"`); computed values are
  labelled in the UI ("computed by SleepLens").
- A "valid night" is usage over 4 hours (240 min), the machine's rule.
- The full serial number never reaches the UI.
- No network access anywhere. The UI loads only bundled assets.

## Version 2 additions (2026-11-09)

Owner feedback after the first real-card test: better usability on Overview,
a period choice (week / month / 6 months / year) on Overview, plain-language
help next to technical words, estimated sleep stages and a 0-100 sleep score,
and suggestions. Everything here is additive; v1 fields stay.

### Changes to existing types

```ts
interface NightListItem {
  // … v1 fields …
  excessLeakP95: number | null;     // L/min, 95th percentile of slots 270-319 (the median is usually 0)
  sleepScore: number | null;        // 0-100, SleepScore.total
  interruptions: number;            // mask-off breaks longer than 1 min inside the night (sessions - 1)
}

interface NightDetail {
  // … v1 fields …
  score: SleepScore | null;
  stages: SleepStageEstimate | null; // null if the night file was not imported
  tips: Tip[];                       // for this night, most important first, max 4
}

interface Trends {
  // … v1 fields …
  avgSleepScore: number | null;
  avgExcessLeakP95: number | null;
  points: { /* v1 */ sleepScore: number | null; excessLeakP95: number | null; prescriptionChanged: boolean }[];
  // prescriptionChanged: mode or pressure settings changed (humidifier/ramp changes are NOT markers)
}
```

### New command

| Command | Args | Returns |
| --- | --- | --- |
| `period_summary` | `{ period: "week" \| "month" \| "6months" \| "year", end?: string }` | `PeriodSummary` (`end` defaults to the latest night; the period is the N days ending on `end`: 7, 30, 182, 365) |

```ts
interface PeriodSummary {
  period: "week" | "month" | "6months" | "year";
  from: string; to: string; daysInRange: number;
  nights: number; validNights: number; compliancePct: number;
  avgUsageMinutes: number | null;
  avgAhi: number | null;
  avgSleepScore: number | null;
  avgExcessLeakP95: number | null;
  avgPressureP90: number | null;
  avgStageMinutes: StageMinutes | null;     // per night with stages
  previous: {                               // same-length period just before; null fields if no data
    avgUsageMinutes: number | null; avgAhi: number | null; avgSleepScore: number | null; compliancePct: number | null;
  };
  bestNight: { date: string; sleepScore: number } | null;
  worstNight: { date: string; sleepScore: number } | null;
  scoreSeries: { date: string; sleepScore: number | null; usageMinutes: number }[];  // one per day in range
  tips: Tip[];                              // for this period, most important first, max 5
}
```

### New types

```ts
type SleepStage = "wake" | "light" | "deep" | "rem" | "noData";

interface StageMinutes { wake: number; light: number; deep: number; rem: number }

interface SleepStageEstimate {
  start: string;                 // time of the first epoch
  epochSeconds: number;          // 30
  stages: SleepStage[];          // one per epoch, gaps are "noData"
  minutes: StageMinutes;
  signalQuality: number;         // 0..1, signal quality (not accuracy)
  estimatorVersion: number;
}

interface SleepScore {
  total: number;                 // 0-100, rounded
  label: "Great" | "Good" | "Fair" | "Poor";   // 85+, 70-84, 50-69, under 50
  estimated: true;               // always: stages are estimated, so the score is too
  components: {
    key: "duration" | "breathing" | "maskSeal" | "continuity" | "sleepStages";
    label: string;               // "Hours used", "Breathing events", "Mask seal", "Unbroken sleep", "Sleep stages (estimated)"
    points: number; maxPoints: number;
    detail: string;              // one plain sentence, e.g. "6 h 58 m, close to the 7 h goal"
  }[];
}

interface Tip {
  id: string;                    // stable, e.g. "short-night", "mask-fit", "ahi-watch"
  severity: "good" | "info" | "watch" | "talk";
  title: string;                 // "Check your mask fit"
  body: string;                  // 1-2 plain sentences; never suggests changing machine settings
  learnSlug: string | null;      // glossary key for "Learn more"
}
```

### Sleep score (computed in sleeplens-core, `score.rs`)

Points (maximum 100):

| Component | Max | Rule (piecewise linear between the points listed) |
| --- | --- | --- |
| Hours used | 30 | 0 h → 0, 4 h → 15, 7 h → 30, more than 7 h → 30 |
| Breathing events (AHI) | 30 | AHI 0-1 → 30, 5 → 22, 15 → 8, 30+ → 0 |
| Mask seal | 15 | large-leak share of the night 0 % → 15, 5 % → 10, 20 % → 0; then minus up to 5 as excess-leak p95 goes from 24 to 40 L/min |
| Unbroken sleep | 10 | 10 minus 2 per interruption, plus minus up to 4 for estimated wake share above 10 % of mask-on time; min 0 |
| Sleep stages (estimated) | 15 | 15 × min(1, (deep + REM) / mask-on sleep ÷ 0.35) |

If a component's data is missing (no AHI, no stages), drop it and scale the
others up to 100. Never score a night under 30 min of use (score `null`).

### Tips (computed in sleeplens-core, `tips.rs`)

Rules, phrased as general guides; never suggest changing pressure or other
machine settings; "talk" tips say to contact the clinician.

- Short night (under 4 h) → watch: keep the mask on all night; Auto ON helps.
- Large leak over 5 % of the night, or excess-leak p95 over 24 L/min → watch: check mask fit and cushion wear.
- AHI 5-15 → watch; AHI over 15, or the same on 3 of the last 7 nights → talk.
- Central apnea index over 5, or more than half of events central with AHI over 5 → talk.
- 3 or more interruptions → info: many mask-off breaks.
- Period compliance under 70 % → watch; 90 % or more → good.
- Period average score up 5+ vs previous → good; down 5+ → info.
- Prescription changed during the night/period → info: compare before and after.
- All good → good: one encouraging line.

### Help terms

The UI shows a small "?" button next to technical words that opens a short
popover (2-3 sentences) with a "More in Learn" link. Keys: `ahi`,
`obstructiveApnea`, `centralApnea`, `hypopnea`, `apneaIndex`, `excessLeak`,
`largeLeak`, `pressureP90`, `pressure`, `ramp`, `comfort`, `validNight`,
`compliance`, `sleepScore`, `sleepStages`, `deepSleep`, `remSleep`,
`lightSleep`, `wake`, `interruptions`. Content lives in the UI (one glossary
file shared by the popovers and the Learn screen).

### Backend notes (v2)

- `AppInfo.needsReread` counts nights without stages from the current
  estimator version. Importing the same card re-reads those night files even
  when their size and modification time have not changed.
- The continuity wake penalty rises linearly from zero at 10% estimated wake
  share to four points at 100%. The sleep-stage component uses light, deep,
  and REM minutes as its sleep denominator, excluding wake.
- Mask seal is omitted when large-leak minutes are missing, even if excess
  leak p95 is available, because the large-leak share is unknown.
- Score components keep their original `maxPoints` weights. Their `points`
  values round to one decimal; `total` rescales the unrounded available
  components to 100 and rounds to an integer.
- A stage estimate with no estimated sleep receives zero stage points.
- Repeated AHI counts the most recent seven distinct imported nights through
  the selected date, including nights separated by calendar gaps.
- An empty database with no explicit period end returns an import-first
  error. An explicit end returns the requested dates, zero nights and null
  averages, with one score-series entry for each day.

## Version 2.1 changes (2026-11-09, after real-card check, design review and QA)

These replace the matching v2 rules.

- **Periods start at the first imported night.** `period_summary` clamps
  `from` to the first night on record (days before it are not "missed"),
  `daysInRange` and compliance use the clamped range, and a new field
  `clampedToFirstNight: boolean` says so. The previous-period comparison is
  null when the previous period would start before the first night.
- **`PeriodSummary.headline: string`**: one plain sentence summarising the
  period, e.g. "A steady month: you used it on 27 of 30 nights, breathing
  events stayed in the normal range, and two nights were harder."
- **`Tip.evidence: string | null`** (one plain line, e.g. "AHI above 5 on 2
  nights") and **`Tip.evidenceDates: string[]`** (nights the evidence refers
  to, max 5, newest first). Period tips must explain themselves this way; a
  period "watch" tip for breathing events only fires when at least 3 nights in
  the period had AHI over 5 (or the period average is over 5).
- **Sleep score points (replaces the v2 table)** — spreads good and great
  nights apart and gives the estimated stages less weight:

  | Component | Max | Rule (piecewise linear) |
  | --- | --- | --- |
  | Hours used | 30 | 0 h → 0, 4 h → 12, 6 h → 22, 7 h → 27, 8 h or more → 30 |
  | Breathing events (AHI) | 30 | 0 → 30, 1 → 28, 2 → 26, 5 → 20, 15 → 8, 30+ → 0 |
  | Mask seal | 15 | large-leak share 0 % → 15, 5 % → 10, 20 % → 0; minus up to 5 as excess-leak p95 goes from 20 to 36 L/min |
  | Unbroken sleep | 15 | 15 minus 3 per interruption, minus up to 6 as estimated wake share goes from 5 % to 20 % of mask-on time; min 0 |
  | Sleep stages (estimated) | 10 | 5 × min(1, deep share ÷ 0.15) + 5 × min(1, REM share ÷ 0.20) |

  Missing components are still dropped and the rest rescaled; labels stay
  (85+ Great, 70-84 Good, 50-69 Fair, under 50 Poor).
