# SleepLens implementation plan

## 1. Goal

A person with a CPAP drags their SD card folder onto a web page and within
seconds understands:

1. **Did I use it enough?** (hours per night, nights over 4 h)
2. **Is it working?** (events per hour, mask leak, pressure)
3. **Is it getting better or worse?** (trends)
4. **What should I ask my doctor?** (a printable summary)

Success means a first-time user can answer those four questions without
reading a manual or knowing what "AHI" means.

## 2. Principles

- **Private by design.** 100% client-side. No backend, no uploads, no
  third-party scripts that see file contents. Works offline once loaded.
- **Plain language first, detail on demand.** Every number has a label a
  non-clinician understands, a status colour and a "what this means" line.
  Raw charts sit one tap deeper.
- **Honest numbers.** If a value is estimated (for example events derived
  from flow because the machine does not store them), say so on screen.
- **Not medical advice.** Persistent, calm disclaimer; never suggest changing
  settings.
- **Pluggable machines.** Each brand is a parser that produces one shared data
  model; the UI never knows which brand it is showing.

## 3. Architecture

```
 drag & drop / file picker / .zip
            │
            ▼
   detect.ts ── picks a parser by folder/file signature
            │
            ▼  (Web Worker, streaming, one night at a time)
   parsers/as2x/*   parsers/resmed/*   parsers/…
            │
            ▼
   model: Device, Night, Session, Channel(Float32Array), Event, SettingsChange
            │
            ├─► metrics.ts   usage, leak, pressure percentiles, events/hour
            │
            ▼
   IndexedDB cache (optional, per browser, user can clear)
            │
            ▼
   UI: Overview · Night · Trends · Report
```

### Stack

| Need | Choice | Why |
| --- | --- | --- |
| App | Vite + React + TypeScript | Simple static build, large contributor pool |
| Styling | CSS modules + design tokens (CSS variables) | Light/dark, no runtime cost |
| Charts | uPlot for waveforms, lightweight SVG for summaries | uPlot draws millions of points smoothly |
| Parsing | Web Worker + `DataView`, streamed with `File.slice()` | 14 GB cards must not freeze or crash the tab |
| Folders | `DataTransferItem.webkitGetAsEntry()` + `<input webkitdirectory>` | Drag a whole card in Chrome, Edge, Safari, Firefox |
| Zip | `fflate` (streaming) | People often zip the card to share |
| Storage | IndexedDB via `idb` | Remember computed summaries, not raw waveforms |
| Tests | Vitest + Playwright | Parser unit tests and drag-drop end-to-end tests |
| Hosting | GitHub Pages via Actions | Free, static, matches "no server" promise |

### Data model (shared by all parsers)

```ts
interface Night {
  date: string;              // evening the night started, YYYY-MM-DD
  sessions: Session[];       // mask-on to mask-off periods
  usageMinutes: number;
  settings: Settings;        // prescribed min/max pressure, ramp, humidity
  events: TherapyEvent[];    // apnea, hypopnea, flow limit…; source: 'device' | 'estimated'
  summary: NightSummary;     // precomputed metrics (cheap to cache)
}
interface Session {
  start: Date; end: Date;
  channels: Partial<Record<'pressure'|'flow'|'leak'|'tidalVolume'|'respRate'|'snore', Channel>>;
}
interface Channel { hz: number; unit: string; data: Float32Array; }
```

### Handling a 14 GB card

- Read the folder tree first (names and sizes only) and show the date range
  instantly.
- Parse the **summary pass** for every night: usage, session times, settings,
  per-night percentiles, computed in the worker with downsampling, not keeping
  full waveforms. Show progress ("Reading night 212 of 689").
- Parse **full waveforms only for the night the user opens**, then drop them
  when they leave.
- Cache summaries in IndexedDB keyed by file name + size + modified time, so a
  second drop of the same card only reads new nights.

## 4. UX and UI

### Research (Mobbin)

| Pattern | Seen in | We take |
| --- | --- | --- |
| One headline score with a labelled breakdown and point weights | Apple Health Sleep Score, Eight Sleep | Nightly "therapy check" with 3-4 contributors, each with its own status |
| Contributor rows with a short bar and a plain status word ("Optimal", "Good") | Oura, Ultrahuman | Usage, mask seal, events, pressure rows; words, not just numbers |
| "Typical range" band behind the value | WHOOP | Show the healthy range (for example under 5 events/hr) as a shaded band |
| Grid of tiles: rating word + metric + small bar | Bevel | Overview tiles on tablet and desktop |
| D / W / M / 6M / Y segmented control | Eight Sleep, Garmin | Trends range switcher |
| Centred dashed drop zone, icon, one primary button, accepted types listed | Fireflies, Asana, Vapi | First screen |
| File list with per-item status ("Scanning…", "Match found") | Revolut Business | Card reading progress per month folder |

### Screens

1. **Drop** (first visit)
   - Headline: "Understand your CPAP data". Sub: "Drag your SD card folder
     here. Nothing leaves your computer."
   - Large dashed drop zone, "Choose folder" button, "Try with sample data"
     link (synthetic night), "How do I copy my SD card?" helper with pictures.
   - Supported machines listed below.
   - States: drag-over highlight, reading with progress, unsupported card
     (shows detected folder names and a "request support" link that prefills
     an issue with names only), partial errors (some nights unreadable).
2. **Overview** (last night by default)
   - Date picker strip (swipe on phone, arrows on desktop).
   - Hero: "Last night: 7 h 12 m. Therapy looked good." with a status ring.
   - Contributor rows/tiles: Hours used · Mask seal (leak) · Breathing
     events per hour · Pressure. Each: value, status word, healthy range band,
     "What this means" disclosure.
   - "Settings changed this night" marker when the log shows one.
3. **Night detail**
   - Timeline of sessions (mask on/off) across the night.
   - Stacked uPlot charts sharing one time axis: pressure, flow, leak, events
     as markers. Pinch/scroll zoom, drag to pan, tap for values.
   - Phone: one chart at a time with a channel switcher.
4. **Trends**
   - Range switcher W / M / 6M / Y / All.
   - Usage calendar heatmap (nights ≥ 4 h), average hours, compliance %
     (the "70% of nights over 4 h" rule many insurers use), events/hour and
     leak lines, settings-change markers.
5. **Report**
   - One printable page (print CSS, also "Save as PDF"): date range,
     compliance, averages, worst nights, settings history, plain-language
     notes and the disclaimer.
6. **Learn** (glossary): AHI, leak, EPR, ramp, central vs obstructive,
   written for patients and linked from every "What this means".

### Responsive rules

- Design at 1920 first; 1280 keeps the same arrangement (side navigation and
  side panels stay beside the content, grids reflow), cards never stretch to
  full width. Stack into columns before stacking into one long column.
- Tablet 768-1024: two-column tiles, charts full width.
- Phone 360-430: single column, bottom tab bar (Overview, Trends, Report,
  Learn), 44 px tap targets, charts one per view, no horizontal page scroll.
- Drag and drop is a desktop idea; on phones the primary action is
  "Choose folder / zip" (Android Chrome supports folders; iOS supports zip
  from Files).
- Light and dark themes from tokens; status colours also carry an icon and a
  word so colour is never the only cue (WCAG AA).
- Motion: none by default. At most a gentle count-up on the hero number and
  the drop-zone highlight, disabled under `prefers-reduced-motion`.

### Metric thresholds (shown, never prescriptive)

| Metric | Good | Watch | Talk to your clinician |
| --- | --- | --- | --- |
| Hours used | ≥ 7 h | 4-7 h | < 4 h |
| Events per hour (AHI) | < 5 | 5-15 | > 15 |
| Large leak time | < 5 % of night | 5-20 % | > 20 % |

Each threshold carries a source link in the glossary and is labelled as a
general guide.

## 5. Phases

### Phase 0: decode the AS2X format (blocker for the author's own card)
- Write a Node script `tools/inspect-as2x.ts` that dumps the record header,
  plots each of the 400-sample slots over a night and guesses channel
  boundaries by spotting smooth vs stepped series.
- Match candidates against known behaviour: pressure steps with the auto
  algorithm, flow oscillates at breathing rate (~0.2-0.3 Hz), leak jumps when
  the mask moves.
- Decode `C5 C5` settings blocks and text events in `.bkp`.
- Create an **anonymised fixture**: serial replaced, dates shifted, one short
  synthetic session generated from the decoded layout.
- Exit: channel map documented in `FORMAT_NOTES.md` with confidence levels.

### Phase 1: skeleton and drop flow
- Vite + React + TS app, tokens, layout shell at all breakpoints.
- Drop zone, folder walking, zip support, worker plumbing, progress.
- Machine detection and the "unsupported card" path.
- Exit: dropping any folder lists recognised nights with dates and sizes.

### Phase 2: AS2X parser and nightly summary
- Streaming parser for `.dat` and `.bkp`, summary pass, IndexedDB cache.
- Overview screen with usage, pressure and leak (events marked "estimated"
  or hidden until Phase 0 answers whether they are stored).
- Exit: the author's 689 nights load, last night's summary matches the
  machine's own screen readout for 5 sampled nights.

### Phase 3: night detail and trends
- uPlot charts with shared zoom; session timeline; trends with heatmap.
- Exit: one night's waveforms render in under 1 s on a mid-range laptop.

### Phase 4: report, glossary, polish
- Print/PDF report, Learn pages, empty and error states, accessibility pass,
  sample-data mode.
- Exit: Lighthouse accessibility ≥ 95; design critique at 1920, 1280, tablet
  and phone with fixes applied.

### Phase 5: ResMed support and release
- EDF+ reader, ResMed `STR.edf` summary and `DATALOG` sessions, mapped to the
  same model. This is the largest user group and the biggest win for others.
- GitHub Pages deploy, README screenshots, `CONTRIBUTING.md` with a "write a
  parser" guide, issue templates (support request with names only).
- Exit: v0.1.0 tagged and live.

### Later / help wanted
- Philips DreamStation, Luna/BMC, Resvent parsers.
- Event detection from flow for machines without stored events (clearly
  labelled as estimated, validated against ResMed nights that do have events).
- PWA install, compare two date ranges, export CSV.

## 6. Testing

- **Parser unit tests** on synthetic and anonymised fixtures only.
- **Golden tests**: summary metrics for fixture nights checked in as JSON.
- **End-to-end** (Playwright): drop a fixture folder, zip and unsupported
  folder; check every screen at 1920, 1280, 768 and 390 widths.
- **Large-card test**: generated 700-night synthetic card to check memory
  stays under ~500 MB and the summary pass finishes in reasonable time.
- **Exploratory pass** before each release: rapid drops, cancelling mid-read,
  back/refresh, keyboard only, dark mode.
- **Privacy check in CI**: fail if any `fetch`/`XMLHttpRequest`/beacon call is
  added outside the static asset loader, and if any `.dat`/`.edf`/`.bkp` file
  is committed.

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| AS2X channel map cannot be fully decoded | Ship usage, sessions and settings first (already clear); label uncertain channels |
| No stored events on AS2X | Show "not recorded by this machine"; flow-based estimate later, labelled |
| Browser memory on huge cards | Streaming summary pass, waveforms only for the open night |
| People read it as diagnosis | Plain disclaimer, no setting advice, thresholds marked as general guides |
| Real data leaking into the repo | `.gitignore`, CI check, fixture anonymiser script |
