# SleepLens desktop backend

Crate: `sleeplens-app`. Run `cd app && cargo tauri dev` with
`/opt/homebrew/opt/rustup/bin` first on PATH. Tauri runs the npm hooks from
`app/`. The UI contract is `../../docs/APP_CONTRACT.md`.

`platform.rs` owns Tauri path resolution and mounted-drive discovery.
`store.rs` owns schema migration version 1, queries and series compression.
`importer.rs` owns the testable import pipeline. `lib.rs` connects these to
Tauri commands and events. Tests use temporary databases and the synthetic
fixture. They never scan mounted volumes or open the user's app database.

The database is `<app_data_dir>/sleeplens.db`. Its tables are `devices`,
`nights`, `sessions`, `events`, `series`, `imports`, and `file_index`.
Nights use the contract's date primary key. Dates shared by different devices
replace the older night. Devices contain only masked serials. File-index paths
replace serial-bearing folders with a SHA-256 device identifier and retain
the relative month/night layout. No raw card data or full serial is stored.
Series are versioned zlib-compressed little-endian f32 channels; NaN gaps
become JSON null when read back.

Unchanged night files count as skipped even when their backup-derived fields
are refreshed. Backup dates outside current log coverage retain their earlier
imported machine fields. Changed night files count as updated. Individual
night errors are reported without aborting other nights. Cancellation takes
effect between nights. Deletion requires `confirm: true` and an idle importer.

Settings history includes the first known snapshot with every key listed as
changed, followed by actual differences. History times are the night's first
mask-on time. Import audit times use local wall-clock time without a zone. Empty all-time trends return empty
`from`/`to` strings, zero counts and null metric averages. Explicit date ranges
include missing calendar days in compliance. Average metrics ignore nulls;
usage averages divide by nights with positive usage. Valid nights exceed
240 minutes strictly.

Only the dialog plugin's `open` permission is enabled. Tauri's open permission
covers both file and directory dialogs; the UI must pass `directory: true`
for folder selection. No filesystem command permissions, HTTP or shell
plugins are enabled. Sample data is mapped from the repository fixture to
`sample-card/` in Tauri resources. The placeholder icon source is
`icons/source.png`.

Verified on macOS on 2026-11-09: `cargo test --workspace` passed 32 tests,
including six app pipeline/store tests; `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo build -p sleeplens-app`, and formatting passed.
`cd app && cargo tauri build --debug --bundles app` passed, including the npm
build hook. Output: `../../target/debug/bundle/macos/SleepLens.app`.
The bundle identity and all four sample resource hashes were verified.
Tauri CLI 2.12.4 was installed. The GUI, Windows and real cards were not tested;
no mounted card was opened and no Git writes were performed.
