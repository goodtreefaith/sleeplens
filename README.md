# SleepLens

**Drop in your CPAP SD card. See how last night went, in plain language.**

SleepLens is a free, open-source desktop app (macOS now, Windows later) for
people who use CPAP and want to understand their therapy data without
learning clinical jargon. Point it at your SD card and get:

- **A nightly summary**: hours used, mask seal, pressure, and events per hour,
  each with a one-line "what this means" and a green / amber / red status.
- **Trends**: usage streaks and averages across weeks, months and years.
- **A detail view of one night**: zoomable pressure, flow and leak charts for
  readers who want to see more.
- **A printable one-page report** to bring to your doctor or sleep technician.

> **Status: early macOS build.** The Rust core and the desktop app (Tauri) work
> with the Aeonmed AS100 series. See
> [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) for the build plan
> and [docs/FORMAT_NOTES.md](docs/FORMAT_NOTES.md) for what we know about the
> card formats.

**Try the demo:** [goodtreefaith.github.io/sleeplens/demo](https://goodtreefaith.github.io/sleeplens/demo/) (or open [demo/index.html](demo/index.html) locally) to see
the planned screens with made-up data (nothing is uploaded).

## Your data stays on your device

SleepLens copies what it needs off the card into a local database on your
computer (`~/Library/Application Support/com.goodtreefaith.sleeplens/` on
macOS), so the card can go straight back into your machine. Nothing is
uploaded: the app has no network access, no account and no analytics. You
can delete everything from Settings.

## Supported machines (planned)

| Machine family | SD card layout | Status |
| --- | --- | --- |
| Aeonmed AS100 series (reports itself as `AS2X`) | `<serial><YYYYMM>/<YYYYMMDD>.dat` + `<serial>.bkp` | Being decoded (first target); format notes cross-checked against Aeonmed's NeoRead reader |
| ResMed AirSense 10 / 11 | `DATALOG/`, `STR.edf` (EDF+) | Planned, format is public |
| Philips DreamStation | `P-Series/` | Later, help wanted |

Got a different machine? Open an issue with the **folder and file names only**
(never the files themselves) and we will help you contribute a parser.

## Not medical advice

SleepLens explains numbers your machine already recorded. It does not diagnose
anything and does not replace your doctor or sleep clinic. Never change your
machine's prescribed settings because of what this app shows; talk to your
clinician first.

## Contributing

See the implementation plan for open tasks. Never commit real therapy data:
the `.gitignore` blocks common card file types, and test fixtures must be
synthetic or fully anonymised (serial numbers, dates and times shifted).

## Licence

[MIT](LICENSE)

## Run the app (macOS)

Needs Rust (`rustup`), Node 20+ and the Tauri CLI (`cargo install tauri-cli --version "^2" --locked`).

```bash
cd app && npm install && cargo tauri dev
```

`cargo tauri build --bundles app` builds `target/release/bundle/macos/SleepLens.app`.
`cargo test --workspace` runs the Rust tests; `cargo run -p sleeplens-cli -- inspect <card>`
prints a card summary from the command line.

## Developer tools

Node 20+ and `npm install` first. Parsers live in `src/parsers/as2x/` (pure
`DataView` code, reusable in the browser). The AS100 format notes in
[docs/FORMAT_NOTES.md](docs/FORMAT_NOTES.md) were cross-checked against how
Aeonmed's own NeoRead software reads the card; no NeoRead code is included or
redistributed, and some fields still need checking on a real card.

- `npm run inspect -- <card folder> [--json] [--night YYYYMMDD]` reads an
  Aeonmed AS100 series card (read-only, one file at a time) and prints one row
  per night: times, usage, pressure, median leak and, when the `.bkp` log still
  has it, AHI and event counts. The serial is always printed as `<SERIAL>`.
  Never paste real output into issues or commits without checking it first.
- `npm run make-fixture` regenerates the fully synthetic card in
  `fixtures/synthetic-as2x/` (fake serial `TEST0000`, dates in 2030). It is the
  only card data allowed in the repo.
- `npm test` runs the parser tests against that fixture; `npm run typecheck`
  runs the strict TypeScript check.
