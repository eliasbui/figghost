#!/bin/sh
# Builds Figghost.app: a menu bar app (no Dock icon) signed with SIGN_IDENTITY (default: ad hoc).
#
# macOS ties the Accessibility grant to the app's code signature. An ad hoc signature changes on every
# build, so the grant is lost each time; sign with a stable local certificate to keep it:
#   SIGN_IDENTITY="Figghost Local" scripts/bundle.sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-"$ROOT/dist"}
IDENTITY=${SIGN_IDENTITY:--}
cd "$ROOT"

cargo build --release -p app-ui
TARGET=$(cargo metadata --format-version 1 --no-deps | python3 -c 'import sys, json; print(json.load(sys.stdin)["target_directory"])')

APP="$OUT/Figghost.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$TARGET/release/app-ui" "$APP/Contents/MacOS/Figghost"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Figghost</string>
    <key>CFBundleDisplayName</key><string>Figghost</string>
    <key>CFBundleIdentifier</key><string>vn.figghost.app</string>
    <key>CFBundleExecutable</key><string>Figghost</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>0.1.0</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSAppleEventsUsageDescription</key><string>Figghost reads the current track from Music and Spotify to show it in the notch.</string>
</dict>
</plist>
PLIST

plutil -lint "$APP/Contents/Info.plist" >/dev/null
codesign --force --sign "$IDENTITY" "$APP"
codesign --verify --strict "$APP"
echo "built $APP (signed with: $IDENTITY)"
