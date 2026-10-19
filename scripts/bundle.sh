#!/bin/sh
# Builds Figghost.app: a menu bar app (no Dock icon) signed with SIGN_IDENTITY (default: ad hoc).
#
# macOS ties the Accessibility grant to the app's designated requirement. The one an ad hoc signature gets is the
# hash of that single build, which no later build meets, so the grant would be lost at every update. An ad hoc
# build is therefore signed to name the app (its identifier) and every later build inherits the grant. That is
# only as strong as the name: with a certificate the grant is pinned to the certificate as well, which a build
# signed by someone else cannot copy:
#   SIGN_IDENTITY="Figghost Local" scripts/bundle.sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-"$ROOT/dist"}
IDENTITY=${SIGN_IDENTITY:--}
cd "$ROOT"

cargo build --release -p app-ui -p nowplaying-adapter
META=$(cargo metadata --format-version 1 --no-deps)
TARGET=$(echo "$META" | python3 -c 'import sys, json; print(json.load(sys.stdin)["target_directory"])')
VERSION=$(echo "$META" | python3 -c 'import sys, json; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "app-ui"))')

APP="$OUT/Figghost.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
cp "$TARGET/release/app-ui" "$APP/Contents/MacOS/Figghost"
# What the media tab shows comes from this library, which /usr/bin/perl loads (crates/nowplaying-adapter); the app
# finds it in Contents/Frameworks and never links it.
cp "$TARGET/release/libfigghost_nowplaying.dylib" "$APP/Contents/Frameworks/"
# Drawn by the make_icon example in macos-sys: cargo run -p macos-sys --example make_icon -- X.iconset,
# then iconutil -c icns X.iconset -o crates/app-ui/assets/Figghost.icns
cp "$ROOT/crates/app-ui/assets/Figghost.icns" "$APP/Contents/Resources/Figghost.icns"
# Inter, the font of the settings page, is inside the executable; its licence travels with the app.
cp "$ROOT/crates/app-ui/assets/Inter-OFL.txt" "$APP/Contents/Resources/Inter-OFL.txt"
# The licence of Figghost itself travels with the app, as the MIT License asks of every copy.
cp "$ROOT/LICENSE" "$APP/Contents/Resources/LICENSE.txt"

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
    <key>NSAudioCaptureUsageDescription</key><string>Figghost changes the volume, output and sound of each app, and records the computer's audio in screen recordings when asked to.</string>
    <key>NSBluetoothAlwaysUsageDescription</key><string>Figghost shows in the notch when a Bluetooth device connects or disconnects.</string>
    <key>NSMicrophoneUsageDescription</key><string>Figghost records the microphone in screen recordings when you turn it on.</string>
    <key>NSCameraUsageDescription</key><string>Figghost shows the camera in screen recordings when you turn it on.</string>
    <key>NSAppleEventsUsageDescription</key><string>Figghost reads the current track from Music and Spotify to show it in the notch.</string>
    <key>NSCalendarsUsageDescription</key><string>Figghost reads your calendar events to show today and tomorrow on the notch and to warn you before a meeting. Nothing leaves this Mac.</string>
    <key>NSCalendarsFullAccessUsageDescription</key><string>Figghost reads your calendar events to show today and tomorrow on the notch and to warn you before a meeting. Nothing leaves this Mac.</string>
    <key>NSRemindersUsageDescription</key><string>Figghost reads your reminders to show the ones that are due on the notch, and marks one complete when you press its button.</string>
    <key>NSRemindersFullAccessUsageDescription</key><string>Figghost reads your reminders to show the ones that are due on the notch, and marks one complete when you press its button.</string>
    <!-- "Add to Figghost Shelf" in the Services menu and in Finder's right click menu (macos-sys/src/services.rs).
         An empty NSRequiredContext makes it on by default. -->
    <key>NSServices</key>
    <array>
        <dict>
            <key>NSMenuItem</key><dict><key>default</key><string>Add to Figghost Shelf</string></dict>
            <key>NSMessage</key><string>addToShelf</string>
            <key>NSPortName</key><string>Figghost</string>
            <key>NSRequiredContext</key><dict/>
            <key>NSSendFileTypes</key><array><string>public.item</string></array>
        </dict>
    </array>
</dict>
</plist>
PLIST

sed -i '' "s/__VERSION__/$VERSION/g" "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
# The Services item in Vietnamese; the key is its default title above.
mkdir -p "$APP/Contents/Resources/vi.lproj"
printf '"Add to Figghost Shelf" = "Th\xc3\xaam v\xc3\xa0o khay Figghost";\n' > "$APP/Contents/Resources/vi.lproj/ServicesMenu.strings"
plutil -lint "$APP/Contents/Resources/vi.lproj/ServicesMenu.strings" >/dev/null
codesign --force --sign "$IDENTITY" "$APP/Contents/Frameworks/libfigghost_nowplaying.dylib"
if [ "$IDENTITY" = - ]; then
    # The same identifier as CFBundleIdentifier above and BUNDLE_ID in crates/app-ui/src/update/install.rs.
    codesign --force --sign - -r='designated => identifier "vn.figghost.app"' "$APP"
    # Fails the build if codesign ever stops honouring it, rather than shipping an app that loses its grants.
    codesign -d -r- "$APP" 2>&1 | grep -qF 'designated => identifier "vn.figghost.app"' ||
        { echo "bundle: the designated requirement was not pinned" >&2; exit 1; }
else
    codesign --force --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"
# Lets the Services menu see the item without logging out; harmless where it cannot.
/System/Library/CoreServices/pbs -update >/dev/null 2>&1 || true
echo "built $APP (signed with: $IDENTITY)"
