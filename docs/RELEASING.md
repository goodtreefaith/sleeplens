# Preparing a macOS release

SleepLens currently targets Apple Silicon Macs. The release helper prepares a
DMG installer, a ZIP containing `SleepLens.app`, checksums and build provenance.
It does not install the app, create a tag, publish a release or push source.
Intel and Windows release builds are not part of this workflow.

## Current signing status

The preview build uses an ad-hoc code signature. This verifies bundle integrity
but does not identify an Apple-verified developer. It is not notarized. A
browser-downloaded copy may be blocked by Gatekeeper. Do not disable Gatekeeper
or remove quarantine protections to distribute or test it.

For normal public macOS distribution, configure Developer ID signing and Apple
notarization before presenting the app as a trusted installer. The current
preview helper deliberately clears Apple account variables in its own process
and forces an ad-hoc signature. It is not the signed-distribution pipeline.
See [Tauri's macOS signing guide](https://v2.tauri.app/distribute/sign/macos/).

## Prepare local artifacts

Finish feature work and review before building a candidate. Freeze the UI,
icons and estimator inputs so the installer matches the final reviewed app.
Run the checks appropriate to those changes, including the Rust workspace,
frontend tests/build and synthetic sample-mode smoke checks.

On a Mac with Node.js 20+, stable Rust, Xcode build tools and Tauri CLI 2.12.4:

```bash
cargo install tauri-cli --version 2.12.4 --locked
rustup target add aarch64-apple-darwin
npm --prefix app ci
scripts/build-macos-release.sh
```

If Homebrew installed Rust outside your shell's PATH, enable that toolchain
first. The helper expects `cargo`, `rustc` and `rustup` on PATH.

The final line names a new directory under `target/release-assets/`. Each run
gets a separate directory and preserves earlier candidates. It contains:

- `SleepLens_<version>_aarch64.dmg`.
- `SleepLens_<version>_aarch64.app.zip`.
- `BUILD_INFO.json` with the commit, dirty state, build-tool versions, signing
  status and SHA256 hashes of the local build inputs.
- `RELEASE_NOTES.md`, a preview description to complete after runtime review.
- `SHA256SUMS` covering those files.

The source snapshot may include uncommitted work. A commit hash in build info
identifies its base; it does not mean the artifact matches that commit alone.
The helper compares build-input hashes before and after packaging and stops
if they change. It checks the bundle identifier/version, synthetic resource hashes, arm64
executable, ad-hoc code signature, DMG, ZIP and final checksums. It disables Finder layout automation during DMG creation. It does not launch
the app or access imported records.
Only the bundled synthetic card is an app resource. Public PSG datasets and
evaluation results remain outside the installer.

## Review the candidate

From the artifact directory, run `shasum -a 256 --check SHA256SUMS`.
Inspect the DMG's app and Applications link without replacing an installed app.
Use a separate synthetic-data app environment for any runtime review; preserve
existing user data and the installed application. Record the tested macOS and
architecture, actual launch/sample-import result, and unresolved issues.

Update the release notes with the final implemented behavior and verified
results. If release notes change, regenerate `SHA256SUMS` for the DMG, ZIP,
`BUILD_INFO.json` and notes, then check it again. Do not describe packaging-only
checks as a tested installation or claim clinical accuracy without held-out
validation evidence.

Review the concrete candidate and final release notes before committing,
tagging or publishing. A public preview must disclose its ad-hoc signature and lack of
notarization.

## Manual GitHub build

`.github/workflows/build-macos.yml` is a manual `workflow_dispatch` job. Once
committed, it can prepare the same Apple Silicon preview and
retain artifacts for 14 days. Existing frontend type checks/tests and Rust
workspace tests gate the build. Real-card tests remain ignored; the workflow
does not opt into private cards or databases. It has read-only repository
permissions and no release or tag publication step. The workflow itself has
not run merely because this file exists.

Download and review the resulting candidate before any separately authorized
GitHub Release. If publishing through GitHub Releases, attach the DMG, app ZIP,
checksums, build info and final notes from the same candidate directory.

## Primary references

- [Tauri DMG distribution](https://v2.tauri.app/distribute/dmg/).
- [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/).
- [Tauri GitHub build pipelines](https://v2.tauri.app/distribute/pipelines/github/).
- [GitHub manual workflows](https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows#workflow_dispatch).
