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
