# SleepLens

**Drop in your CPAP SD card. See how last night went, in plain language.**

SleepLens is a free, open-source web app for people who use CPAP and want to
understand their therapy data without installing desktop software or learning
clinical jargon. Drag the SD card folder (or a zip of it) onto the page and get:

- **A nightly summary**: hours used, mask seal, pressure, and events per hour,
  each with a one-line "what this means" and a green / amber / red status.
- **Trends**: usage streaks and averages across weeks, months and years.
- **A detail view of one night**: zoomable pressure, flow and leak charts for
  readers who want to see more.
- **A printable one-page report** to bring to your doctor or sleep technician.

> **Status: planning.** No app code yet. See
> [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) for the build plan
> and [docs/FORMAT_NOTES.md](docs/FORMAT_NOTES.md) for what we know about the
> card formats.

**Try the demo:** [goodtreefaith.github.io/sleeplens/demo](https://goodtreefaith.github.io/sleeplens/demo/) (or open [demo/index.html](demo/index.html) locally) to see
the planned screens with made-up data (nothing is uploaded).

## Your data stays on your device

SleepLens is a static site. Your files are read and analysed **inside your
browser**. Nothing is uploaded, there is no account, no analytics on file
contents and no server. You can save the page and run it offline.

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
