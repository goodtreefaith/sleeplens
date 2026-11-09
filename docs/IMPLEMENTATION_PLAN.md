# SleepLens implementation plan

## 1. Goal

A person with a CPAP opens the SleepLens desktop app, points it at their SD
card (or a copy of it) and within seconds understands:

1. **Did I use it enough?** (hours per night, nights over 4 h)
2. **Is it working?** (events per hour, mask leak, pressure)
3. **Is it getting better or worse?** (trends)
4. **What should I ask my doctor?** (a printable summary)

Success means a first-time user can answer those four questions without
reading a manual or knowing what "AHI" means.

**Platform decision (2026-11-09):** SleepLens is a native desktop app built
in Rust, macOS first, with Windows support planned (not built yet). The
earlier plan for a static web app is replaced. Data lives on the user's own
computer: the app copies what it needs off the card into a local database, so
the card can go straight back into the machine.

## 2. Principles

- **Private by design.** Everything runs on the user's computer. No server,
  no account, no telemetry, no network calls at all. The app works offline.
- **Your data stays on your device.** Imported nights are stored in a local
  SQLite database in the operating system's app-data folder
  (`~/Library/Application Support/SleepLens` on macOS,
  `%APPDATA%\SleepLens` on Windows). The user can see the folder, export it
  and delete it from Settings.
- **Plain language first, detail on demand.** Every number has a label a
  non-clinician understands, a status colour and a "what this means" line.
  Raw charts sit one tap deeper.
- **Honest numbers.** Prefer the machine's own summary values (they match its
  screen); say so on screen when a value is computed by SleepLens instead.
- **Not medical advice.** Persistent, calm disclaimer; never suggest changing
  settings.
- **Pluggable machines.** Each brand is a parser that produces one shared data
  model; the UI never knows which brand it is showing.
- **Cross-platform by construction.** No macOS-only APIs in the core or the
  app logic; anything platform-specific sits behind a small module so the
  Windows build is a packaging task, not a rewrite.

## 3. Architecture

```
 SD card folder (or a copy / zip)  ── chosen with the native folder picker
            │
            ▼
 sleeplens-core (Rust crate, no UI, no OS calls)
   detect ── picks a parser by folder/file signature
   parsers::as2x   parsers::resmed (later)   …
   model: Device, Night, Session, Settings, Event, Series
   metrics: usage, leak, pressure percentiles, events/hour
            │
            ▼
 SleepLens app (Tauri 2, Rust backend)
   import ── walks the card, parses one night at a time on a worker thread,
             reports progress, skips nights already imported
   store  ── SQLite (rusqlite): devices, nights, summaries, events, settings
             history, 1 Hz series per night
   commands ── typed IPC the UI calls (list nights, night detail, trends …)
            │
            ▼
 UI (web view inside the app: Vite + React + TypeScript)
   Import · Overview · Night · Trends · Report · Settings
```

Why Tauri: the core and all data handling are Rust, the app bundle is small
(the system web view, no bundled browser), and the same project builds a
`.app`/`.dmg` on macOS and an `.msi` on Windows. The UI reuses the design
already worked out in `demo/`. The web view loads only local files; a strict
Content Security Policy blocks all network access.

### Repository layout

```
Cargo.toml                 Rust workspace
crates/sleeplens-core/     parsers, data model, metrics (+ tests on fixtures)
crates/sleeplens-cli/      `sleeplens inspect <card>` (replaces the Node inspector)
app/                       Tauri app
  src-tauri/               Rust backend: import, store, commands
  src/                     React UI
fixtures/synthetic-as2x/   synthetic test card (shared by Rust and TS tests)
src/, tools/, tests/       TypeScript reference parser, kept until the Rust
                           port matches it on the fixture and the real card
```

### Stack

| Need | Choice | Why |
| --- | --- | --- |
| Core | Rust (stable), `sleeplens-core` crate | Fast, safe binary parsing; one implementation for every platform |
| Desktop shell | Tauri 2 | Rust backend, system web view, macOS and Windows from one project |
| UI | Vite + React + TypeScript inside the web view | Reuses the demo design; large contributor pool |
| Styling | CSS variables (design tokens) | Light/dark, follows the system theme |
| Charts | uPlot for waveforms, SVG for summaries | Draws a whole night of 1 Hz data smoothly |
| Storage | SQLite via `rusqlite` (bundled) | One local file, easy to back up or delete |
| Reading files | `std::fs` + memory-mapped or chunked reads | 30 MB night files, 14 GB cards, never all in memory |
| Zip import | `zip` crate | People often zip the card to share |
| Tests | `cargo test` (core, store), Vitest (UI logic), WebDriver/Playwright against the app for end-to-end | |
| Packaging | `tauri build` → `.dmg` (macOS, Apple Silicon and Intel), later `.msi` (Windows) | |

### Data model (shared by all parsers, Rust)

```rust
struct Night {
    date: NaiveDate,             // evening the night started
    sessions: Vec<Session>,      // mask-on to mask-off periods
    usage_minutes: f32,
    settings: Settings,          // mode, start/min/max pressure, ramp, humidity, comfort
    events: Vec<TherapyEvent>,   // obstructive/central apnea, hypopnea, leak level …
    summary: NightSummary,       // machine summary when present, else computed (with source flag)
}
struct Series { hz: f32, unit: &'static str, data: Vec<f32> }  // pressure, flow amplitude, excess/total leak, snore
```

### What is stored, and what is not

- Stored per night: summary, sessions, events, settings snapshot and 1 Hz
  series (pressure, excess leak, total leak, flow amplitude, snore). About
  0.5-1 MB per night, so ~700 nights fit in well under 1 GB.
- Not stored by default: the raw 50 Hz waveforms (18-32 MB per night). The
  night view shows 1 Hz data; when the card or a card copy is available it
  can load the full 50 Hz waveform for a zoomed-in window. An optional
  "Keep full copies of night files" setting can archive them later.
- Re-importing the same card only reads new or changed nights (keyed by file
  name, size and modified time).

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

#### Score, sleep stages and period views (Mobbin, pulled 2026-11-09)

The sleep score, estimated stages and Week/Month/6 months/Year views follow
these references. Check new UI against them, not against general taste.

| Pattern | Reference | We take |
| --- | --- | --- |
| Score ring split into coloured segments, one per component; big status word beside it; component rows "Duration 40/50" with a one-line reason, inside the same card | [Apple Health Sleep Score](https://mobbin.com/screens/8a81ce54-1ba8-4f72-b939-962e73e72e00), [summary](https://mobbin.com/screens/7aecb44c-8dfc-46e9-883e-86eb5301ea19) | Score card = segmented ring + "Great" + component rows visible (not hidden) |
| Big number + status word ("73 GOOD"), then a "Contributors" list: each row a bar + value or word ("Optimal", "Fair") + chevron to detail | [Oura sleep](https://mobbin.com/screens/ded88fa6-85b6-460f-89b9-9eb71fcd2ef9), [contributors](https://mobbin.com/screens/655447d8-888f-447a-8566-9a294f382f98) | Each score component shows a status word, not only points |
| Score ring + rows with a 3-step mini meter (Poor / Sufficient / Optimal) and a legend | [WHOOP sleep performance](https://mobbin.com/screens/7734a2ec-4e6f-4923-9d3c-a3d9d84dd619) | 3-step meters on component rows |
| Stepped hypnogram: stages on rows (Awake top, then REM, Light, Deep), blocks joined by vertical connector lines, start/end times under it, legend below | [Oura stages](https://mobbin.com/screens/b7ae7e96-85bc-440b-8f7f-e7450f7523fd), [Eight Sleep](https://mobbin.com/screens/1950371b-dbdc-4144-a424-049b44c06602), [Ultrahuman](https://mobbin.com/screens/aa610b78-5ec6-44fb-9b10-0b98c59f7adb), [Bevel](https://mobbin.com/screens/c4c2dd72-1d95-431b-898f-ad415b4f5c39) | Hypnogram style for Overview and the Night lane |
| Stage summary list: stage name, horizontal bar, % and duration, with a shaded "typical range" on each bar | [The Outsiders](https://mobbin.com/screens/88396821-5f55-46d9-8c1e-fc4bc91be47b), [Bevel tiles](https://mobbin.com/screens/c4c2dd72-1d95-431b-898f-ad415b4f5c39) | Stage minutes as bars with typical-range bands |
| Sleep cycles count under the hypnogram | [Ultrahuman](https://mobbin.com/screens/aa610b78-5ec6-44fb-9b10-0b98c59f7adb) | "About N sleep cycles (estimated)" |
| D / W / M / Y segmented control, "This week" title with prev/next, big average score + one-sentence interpretation, chart with dotted Poor/Fair/Good/Excellent guide lines | [Google Health (Fitbit)](https://mobbin.com/screens/271b5a7b-fd66-43d3-acc0-6bd63c812f96), [Oura week](https://mobbin.com/screens/b6a702b9-cf15-4ff0-bae5-e68ceaabfa29), [Garmin](https://mobbin.com/screens/690bc0dd-48c5-4d95-8337-ca094006ca55) | Period view header, headline sentence, score chart with guide lines |
| One bar per night with the value above it; stacked deep + REM "restorative sleep" bars | [WHOOP period](https://mobbin.com/screens/85171de8-b0a9-4930-8ac0-8964d7af355c) | Week/Month score and stage charts as bars |
| Best and worst session side by side, "compared with" previous period | [Pillow](https://mobbin.com/screens/cd955e86-9e0e-4860-b6bd-5eed6cf39652) | Best/lowest night pair and period comparison |
| (i) on each card opening a short explanation; an "About" section | [Apple Health about](https://mobbin.com/screens/38b2d05a-63fc-4983-b179-e8ec1eec2890), [WHOOP](https://mobbin.com/screens/595aadc4-d5f7-4a4e-84a0-0c56fca951e1) | Help popovers on technical words; "About the score" text |

### Screens

1. **Import** (first launch)
   - Headline: "Understand your CPAP data". Sub: "Choose your SD card. Your
     data is copied to this computer and never leaves it."
   - Drop zone (drag the card or folder onto the window), "Choose SD card…"
     button using the native folder picker, "Try with sample data" link,
     and a hint when an SD card is detected under `/Volumes`.
   - After import: "You can put the card back in your machine."
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
6. **Settings**: data folder location, export, delete all data, theme,
   pressure unit display.
7. **Learn** (glossary): AHI, leak, EPR, ramp, central vs obstructive,
   written for patients and linked from every "What this means".

### Responsive rules

- Design at 1920 first; 1280 keeps the same arrangement (side navigation and
  side panels stay beside the content, grids reflow), cards never stretch to
  full width. Stack into columns before stacking into one long column.
- Desktop window sizes: minimum window 960 × 640; narrow windows (under
  ~900 px) collapse the side navigation to icons and stack side panels below
  the content.
- No phone layout: SleepLens is a desktop app. (A phone companion is out of
  scope.)
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

### Phase 0: decode the Aeonmed AS100 (AS2X) format (blocker for the author's own card)
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

Resolved by cross-checking against Aeonmed's NeoRead reader (see
`FORMAT_NOTES.md`, "Confirmed (NeoRead)"):
- Model: `AS2X` is an AS100A (auto CPAP); `.dat` byte 8 gives the record
  format version, and the header holds firmware and hardware versions.
- Channels: slots 70-119 measured pressure, 170-219 snore, 220-269 flow,
  270-319 leak flow.
- `.bkp`: fixed address map, summary fields (start pressure, large-leak
  minutes, SNI, central apnea index) and the full `C5 C5` settings list
  (mode, start/min/max pressure, ramp, comfort, Auto ON/OFF, pressure unit).
- Events: `E1` obstructive, `E2` central; `E4`-`E7`, `EE`, `EF` likely
  follow NeoRead's order.
- Report rules: a valid day is over 4 hours; morning event blocks belong to
  the previous date.

Checked on the real card (2026-11-09): every value on the machine's
"Last night" screen matches the `.bkp` summary; pressure unit 1 = hPa;
LEAK90 behaves as minutes above 90 L/min. Still open: `E4`-`E6` (never seen on
this firmware) and a night with a real large leak.

### Phase 1: Rust core
- `sleeplens-core`: port the AS2X parser (`.dat`, `.bkp`, settings,
  detection) from the TypeScript reference, with the NeoRead and
  machine-screen findings.
- `sleeplens-cli inspect` matching the Node inspector's output.
- Exit: `cargo test` passes on the synthetic fixture and the CLI's numbers
  match the TypeScript inspector on the real card (all 690 nights).

### Phase 2: desktop app skeleton (macOS)
- Tauri 2 project in `app/`, strict CSP, no network permissions.
- Import flow: native folder picker and drag-onto-window, background import
  thread with progress events, SQLite store in the app-data folder,
  incremental re-import.
- Typed commands: `import_card`, `import_progress`, `list_nights`,
  `night_detail`, `trends`, `device_info`, `settings_history`,
  `data_location`, `delete_all_data`.
- Exit: importing the real card stores 690 nights; last night's summary in the
  app equals the machine's screen.

### Phase 3: UI
- Overview, Night (uPlot, 1 Hz series, events as markers, session
  timeline), Trends (range switcher, usage calendar, AHI/leak lines,
  settings-change markers), Settings, Learn; built from the demo design.
- Exit: design critique at 1920 and 1280 window widths and a narrow window,
  light and dark, fixes applied; exploratory test pass.

### Phase 4: report and polish
- Printable one-page report (print to PDF from the web view), empty and error
  states, accessibility pass (keyboard, VoiceOver, contrast), sample-data
  mode, app icon, signed and notarised `.dmg`.
- Exit: v0.1.0 `.dmg` for macOS.

### Phase 5: Windows (future)
- Build and test the same app on Windows 10/11 (WebView2), `.msi` installer,
  SD card detection by removable drive letters, `%APPDATA%` data folder,
  code signing.
- Exit: the same fixture and a real card import on Windows.

### Phase 6: ResMed support
- EDF+ reader, ResMed `STR.edf` summary and `DATALOG` sessions in
  `sleeplens-core`, mapped to the same model.

### Later / help wanted
- Philips DreamStation, Luna/BMC, Resvent parsers.
- Event detection from flow for machines without stored events (clearly
  labelled as estimated, validated against ResMed nights that do have events).
- Compare two date ranges, export CSV, optional archive of raw night files.

## 6. Testing

- **Core unit tests** (`cargo test`) on synthetic and anonymised fixtures
  only; golden JSON for fixture nights shared with the TypeScript tests.
- **Parity check**: the Rust CLI and the TypeScript inspector must print the
  same numbers on the real card before the TypeScript code is retired.
- **Store tests**: import the fixture twice; the second import adds nothing.
- **App end-to-end**: import the fixture, visit every screen at 1920, 1280
  and a narrow window, light and dark.
- **Large-card test**: generated 700-night synthetic card; import stays under
  ~500 MB memory and shows steady progress.
- **Exploratory pass** before each release: cancel mid-import, re-import,
  remove the card during import, keyboard only, dark mode.
- **Privacy check**: the Tauri config grants no network or shell
  permissions, the CSP blocks remote origins, and CI fails if any
  `.dat`/`.edf`/`.bkp` file outside the synthetic fixture is committed.

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| AS2X channel map cannot be fully decoded | Ship usage, sessions and settings first (already clear); label uncertain channels |
| No stored events on AS2X | Show "not recorded by this machine"; flow-based estimate later, labelled |
| Memory on huge cards | One night parsed at a time on a worker thread; only 1 Hz series kept |
| Windows differences (paths, WebView2, drive letters) | No platform APIs in core; platform code behind one module; Windows CI build from Phase 2 |
| Unsigned app warnings on macOS | Sign and notarise the `.dmg` in Phase 4 |
| People read it as diagnosis | Plain disclaimer, no setting advice, thresholds marked as general guides |
| Real data leaking into the repo | `.gitignore`, CI check, fixture anonymiser script |
