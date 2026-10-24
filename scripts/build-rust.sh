#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS required' >&2; exit 64; }
[[ "$EUID" != 0 ]] || { echo 'Build as the normal user, without sudo' >&2; exit 64; }
# A pin represents precisely one native architecture.
case "$(uname -m)" in arm64) native_target=aarch64-apple-darwin;; x86_64) native_target=x86_64-apple-darwin;; *) exit 64;; esac
unset CARGO_BUILD_TARGET
export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:--}"
edition="${1:-personal}"
case "$edition" in personal|public) ;; *) echo 'usage: build-rust.sh personal|public' >&2; exit 64;; esac
export VITE_EDITION="$edition"
cargo build --locked --release --target "$native_target" -p switcher-core --bin claude-switcher
cargo run --locked -p switcher-domain --bin export-types
cd apps/desktop
npm ci --ignore-scripts
npm run check
# Tauri forwards Cargo-only flags after the argument separator.
npm run tauri -- build --target "$native_target" --features "$edition" --config "../../packaging/editions/$edition.json" -- --no-default-features
