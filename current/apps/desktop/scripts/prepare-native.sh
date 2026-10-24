#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$project_root"
[[ "$(uname -s)" == Darwin && "$EUID" != 0 ]] || { echo 'Build on macOS as the normal user' >&2; exit 64; }
case "$(uname -m)" in arm64) native_target=aarch64-apple-darwin;; x86_64) native_target=x86_64-apple-darwin;; *) exit 64;; esac
component_directory="$project_root/apps/desktop/src-tauri/components"
mkdir -p "$component_directory"
signing_identity="${APPLE_SIGNING_IDENTITY:--}"
stage="$(mktemp -d "${TMPDIR:-/tmp}/switcher-native.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
python3 - "$stage" <<'PY'
import json, pathlib, plistlib, sys
version = json.loads(pathlib.Path('apps/desktop/src-tauri/tauri.conf.json').read_text())['version']
for role in ['helper', 'guardian']:
    (pathlib.Path(sys.argv[1])/(role+'.plist')).write_bytes(plistlib.dumps(dict(
        CFBundleIdentifier='local.claudeswitcher.'+role,
        CFBundleShortVersionString=version, CFBundleVersion=version)))
PY
for component in helper guardian; do
  package=switcher-helper
  if [[ "$component" == guardian ]]; then package=switcher-core; fi
  cargo rustc --locked --release --target "$native_target" -p "$package" --bin "switcher-$component" -- \
    -C "link-arg=-Wl,-sectcreate,__TEXT,__info_plist,$stage/$component.plist"
  install -m 755 "$project_root/target/$native_target/release/switcher-$component" "$component_directory/switcher-$component"
  args=(--force --options runtime --sign "$signing_identity" --identifier "local.claudeswitcher.$component")
  if [[ "$signing_identity" != "-" ]]; then args+=(--timestamp); fi
  /usr/bin/codesign "${args[@]}" "$component_directory/switcher-$component"
  /usr/bin/codesign --verify --strict "$component_directory/switcher-$component"
done

# The compatibility entrypoint executes this Rust CLI from the installed App.
cargo build --locked --release --target "$native_target" -p switcher-core --bin claude-switcher
install -m 755 "$project_root/target/$native_target/release/claude-switcher" "$component_directory/claude-switcher"
/usr/bin/codesign --force --options runtime --sign "$signing_identity" --identifier local.claudeswitcher.cli "$component_directory/claude-switcher"
/usr/bin/codesign --verify --strict "$component_directory/claude-switcher"
