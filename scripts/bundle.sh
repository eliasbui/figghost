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
META=$(cargo metadata --format-version 1 --no-deps)
TARGET=$(echo "$META" | python3 -c 'import sys, json; print(json.load(sys.stdin)["target_directory"])')
VERSION=$(echo "$META" | python3 -c 'import sys, json; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "app-ui"))')

APP="$OUT/Figghost.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/app-ui" "$APP/Contents/MacOS/Figghost"
# Drawn by the make_icon example in macos-sys: cargo run -p macos-sys --example make_icon -- X.iconset,
# then iconutil -c icns X.iconset -o crates/app-ui/assets/Figghost.icns
cp "$ROOT/crates/app-ui/assets/Figghost.icns" "$APP/Contents/Resources/Figghost.icns"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Figghost</string>
    <key>CFBundleDisplayName</key><string>Figghost</string>
    <key>CFBundleIdentifier</key><string>vn.figghost.app</string>
    <key>CFBundleExecutable</key><string>Figghost</string>
    <key>CFBundleIconFile</key><string>Figghost</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>__VERSION__</string>
    <key>CFBundleShortVersionString</key><string>__VERSION__</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSAppleEventsUsageDescription</key><string>Figghost reads the current track from Music and Spotify to show it in the notch.</string>
    <key>NSCalendarsUsageDescription</key><string>Figghost reads your calendar events to show today and tomorrow on the notch and to warn you before a meeting. Nothing leaves this Mac.</string>
    <key>NSCalendarsFullAccessUsageDescription</key><string>Figghost reads your calendar events to show today and tomorrow on the notch and to warn you before a meeting. Nothing leaves this Mac.</string>
    <key>NSRemindersUsageDescription</key><string>Figghost reads your reminders to show the ones that are due on the notch, and marks one complete when you press its button.</string>
    <key>NSRemindersFullAccessUsageDescription</key><string>Figghost reads your reminders to show the ones that are due on the notch, and marks one complete when you press its button.</string>
</dict>
</plist>
PLIST

sed -i '' "s/__VERSION__/$VERSION/g" "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
codesign --force --sign "$IDENTITY" "$APP"
codesign --verify --strict "$APP"
echo "built $APP (signed with: $IDENTITY)"
