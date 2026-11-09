#!/usr/bin/env bash
# Prepare local Apple Silicon preview artifacts. This script never installs or publishes them.
set -euo pipefail

repo="$(cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo"
[[ "$(uname -s)" == Darwin ]] || { echo 'Build on macOS.' >&2; exit 1; }
for tool in cargo rustc rustup npm node python3 codesign hdiutil ditto shasum unzip lipo git cmp; do
  command -v "$tool" >/dev/null || { echo "Missing build tool: $tool" >&2; exit 1; }
done
target=aarch64-apple-darwin
rustup target list --installed | grep -x "$target" >/dev/null || { echo "Install the Rust target with: rustup target add $target" >&2; exit 1; }
version="$(python3 -c 'import json; print(json.load(open("app/src-tauri/tauri.conf.json"))["version"])')"
target_dir="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
mkdir -p "$target_dir/release-assets"
out="$(mktemp -d "$target_dir/release-assets/SleepLens_${version}_${target}.XXXXXX")"

snapshot() {
  python3 - "$repo" <<'PY'
import hashlib,json,pathlib,sys
r=pathlib.Path(sys.argv[1]);files=[]
for name in ('Cargo.toml','Cargo.lock','app/package.json','app/package-lock.json','app/index.html','app/vite.config.ts','app/tsconfig.json','scripts/build-macos-release.sh'):
    p=r/name
    if p.is_file():files.append(p)
for name in ('app/src','app/public','app/src-tauri','crates/sleeplens-core','fixtures/synthetic-as2x'):
    for p in (r/name).rglob('*'):
        if p.is_file() and not any(x in ('target','node_modules','gen') for x in p.relative_to(r).parts):files.append(p)
print(json.dumps({str(p.relative_to(r)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(files))},indent=2,sort_keys=True))
PY
}
source_commit="$(git rev-parse HEAD)"
snapshot > "$out/source-before.json"

# Keep this preview independent of any Apple account credentials in the caller's environment.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD
export APPLE_SIGNING_IDENTITY=-
# Skip Finder layout automation while packaging the disk image.
unset TAURI_BUNDLER_DMG_IGNORE_CI
export CI=true
(
  cd app
  cargo tauri build --ci --target "$target" --bundles app,dmg --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}' -- --locked
)
[[ "$(git rev-parse HEAD)" == "$source_commit" ]] || { echo 'Source commit changed during packaging. Rebuild the candidate.' >&2; exit 1; }
snapshot > "$out/source-after.json"
cmp -s "$out/source-before.json" "$out/source-after.json" || { echo 'Build inputs changed during packaging. Do not distribute these artifacts.' >&2; exit 1; }

bundle="$target_dir/$target/release/bundle"
app="$bundle/macos/SleepLens.app"
dmg="$bundle/dmg/SleepLens_${version}_aarch64.dmg"
[[ -d "$app" && -s "$dmg" ]] || { echo 'Expected app bundle or DMG is missing.' >&2; exit 1; }
python3 - "$app" "$repo" <<'PY_CHECK'
import hashlib,json,pathlib,plistlib,sys
app,r=map(pathlib.Path,sys.argv[1:]);config=json.loads((r/'app/src-tauri/tauri.conf.json').read_text());info=plistlib.loads((app/'Contents/Info.plist').read_bytes())
assert info['CFBundleIdentifier']==config['identifier'], 'Bundle identifier mismatch'
assert info['CFBundleShortVersionString']==config['version'], 'Bundle version mismatch'
fixture=r/'fixtures/synthetic-as2x'
for source in fixture.rglob('*'):
    if source.is_file():
        bundled=app/'Contents/Resources/sample-card'/source.relative_to(fixture)
        assert bundled.is_file() and hashlib.sha256(source.read_bytes()).digest()==hashlib.sha256(bundled.read_bytes()).digest(), 'Synthetic resource mismatch'
PY_CHECK
codesign --verify --deep --strict "$app"
codesign --display --verbose=2 "$app" 2>&1 | grep -x 'Signature=adhoc' >/dev/null || { echo 'Expected an ad-hoc preview signature.' >&2; exit 1; }
[[ "$(lipo -archs "$app/Contents/MacOS/sleeplens-app")" == arm64 ]] || { echo 'Expected an arm64 executable.' >&2; exit 1; }
hdiutil verify "$dmg"
cp "$dmg" "$out/"
ditto -c -k --sequesterRsrc --keepParent "$app" "$out/SleepLens_${version}_aarch64.app.zip"
unzip -t "$out/SleepLens_${version}_aarch64.app.zip" >/dev/null

python3 - "$repo" "$out" "$version" "$target" "$source_commit" <<'PY'
import datetime,json,pathlib,re,subprocess,sys
r,out,version,target,source_commit=sys.argv[1:];out=pathlib.Path(out)
def run(*args):return subprocess.check_output(args,cwd=r,text=True).strip()
dirty=bool(run('git','status','--porcelain'))
stage_source=(pathlib.Path(r)/'crates/sleeplens-core/src/sleep_stages/mod.rs').read_text()
estimator_version=int(re.search(r'pub const ESTIMATOR_VERSION:\s*u32\s*=\s*(\d+);',stage_source).group(1))
manifest={
    'product':'SleepLens','version':version,'target':target,
    'builtAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'sourceCommit':source_commit,'sourceDirty':dirty,
    'sourceState':('Uncommitted local snapshot based on sourceCommit.' if dirty
                   else 'Clean committed snapshot at sourceCommit. No release tag is asserted.'),
    'sourceSha256':json.loads((out/'source-before.json').read_text()),
    'estimatorVersion':estimator_version,
    'signing':'ad-hoc','developerIdSigned':False,'notarized':False,
    'rust':run('rustc','--version'),'tauriCli':run('cargo','tauri','--version'),
    'node':run('node','--version'),'macOS':run('sw_vers','-productVersion'),
    'runtimeTested':False
}
(out/'BUILD_INFO.json').write_text(json.dumps(manifest,indent=2)+'\n')
(out/'RELEASE_NOTES.md').write_text(f'''# SleepLens {version} preview

Apple Silicon macOS preview. Intel and Windows builds are not included.

- Import an Aeonmed AS100 card into local storage and inspect nightly therapy data.
- Explore the interface with bundled synthetic sample data.
- See the new owl branding and score artwork.
- Save an optional Rhythm profile from your own sleep preferences, sleep window
  and chosen focus window. The profile describes your answers, not a measured
  biological rhythm. Matching fox artwork illustrates the preference you select.
- Includes version {estimator_version} of the experimental sleep-stage estimator.

This build has an ad-hoc signature. It is not signed with an Apple Developer ID
and is not notarized. macOS may block a downloaded copy. Do not disable
Gatekeeper or remove quarantine protections to run it.

Sleep scores and breathing-based stage estimates are experimental. They do not
measure sleep stages or replace a sleep study or clinical advice.

Packaging checks cover archive integrity, architecture and code-signature
integrity. A separate QA identifier/debug build may receive a native runtime
smoke check. That does not establish a runtime test of this exact release
artifact. BUILD_INFO.json records this artifact's source snapshot and runtime
status. Review final feature/evaluation results before publishing.
''')
(out/'source-before.json').unlink();(out/'source-after.json').unlink()
PY
(
  cd "$out"
  shasum -a 256 ./*.dmg ./*.zip BUILD_INFO.json RELEASE_NOTES.md > SHA256SUMS
  shasum -a 256 --check SHA256SUMS
)
printf 'Prepared artifacts: %s\n' "$out"
printf 'Ad-hoc signed, not notarized. No installation, tagging or publication performed.\n'
