# sleeplens-core

Offline Aeonmed AS2X parsing and app-ready night assembly. No network calls.
The optional `fs` feature is enabled by default. Disable default features to
supply your own file source without using the filesystem implementation.

```rust,no_run
use sleeplens_core::{stream_card, FsCardSource};

let source = FsCardSource::new("/path/to/card-copy");
let metadata = stream_card(&source, |night| {
    // Persist each Night here. JSON field names match docs/APP_CONTRACT.md.
    println!("{}: {} minutes", night.list.date, night.list.usage_minutes);
})?;
# Ok::<(), std::io::Error>(())
```

`CardSource::list()` returns relative file names and `read(path)` returns one
file's bytes. `FsCardSource` uses `std::fs` and confines reads to its chosen
root. `detect_as2x` recognises names without reading file contents.
`stream_detected_card` accepts an existing detection result. Both streaming
functions read the backup log once, then read and deliver one night at a time.
`assemble_card` collects the same records into `CardImport` for smaller cards.

`Night` contains a flattened `NightListItem` in `list`, sessions, machine
summary fields, an optional `Settings` snapshot, mapped `TherapyEvent` records,
and aligned 1 Hz `Series`. `NightDetail` is an alias for `Night`. Dates use the
evening rule, with times before noon assigned to the preceding day. Series
contain `Option<f32>` values, so clock skips and mask-off gaps become JSON
`null`. A corrupt night spanning more than 48 hours returns an error before
allocating series. Machine usage and pressure take precedence over computed
values, with `Source` recording which was used. Missing machine indices stay
null. `CardMetadata::device` includes only a masked serial.

For low-level inspection use `dat::{parse_dat_header, iterate_dat_records,
summarize_dat}`, `bkp::{parse_bkp, parse_therapy_entry, event_type_for_code}`,
and `settings::parse_settings_block`. These preserve the reference parser's
fields, including raw headers/settings and unmasked serials. Keep these
low-level records out of UI responses. `parsers::as2x` also reexports the
parser functions and types.

Verified 2026-11-09 on the committed synthetic fixture: 26 workspace tests,
workspace clippy with warnings denied, formatting, and a core build without
default features passed. Inspector stdout and parsed JSON match the TypeScript
reference, including the paused night and the missing-backup text fallback.
Real-card parity and Windows execution remain untested. This port was prepared
within Faith's core/CLI-only scope; no Git writes or app changes were made.
