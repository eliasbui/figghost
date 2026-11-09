#!/bin/sh
# A trial build for this Mac: the app, signed ad hoc with a requirement that names the bundle and not its hash, so that the
# permissions the system grants it (Accessibility, Screen Recording) survive a rebuild. It is not notarized.
# Usage: tools/bundle-local.sh   (from anywhere; prints the path of the app)
set -eu
cd "$(dirname "$0")/.."
pnpm tauri build --bundles app
APP="src-tauri/target/release/bundle/macos/Figghost Tauri.app"
codesign --force --sign - -r='designated => identifier "vn.figghost.tauri"' "$APP"
codesign --verify --strict "$APP"
echo "$PWD/$APP"
