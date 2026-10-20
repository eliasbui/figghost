#!/bin/sh
# Starts the real program on a private instance with the live islands on and checks, through its trace, that
# a download that finishes puts an island on the notch and a file a browser is still writing does not, and that
# a drive that is mounted and unmounted does. The notch is placed outside every screen and the settings window
# stays closed. Nothing of the real settings or the real Downloads folder is touched.
#
# Usage: scripts/check-islands.sh [program]    (default: target/debug/app-ui)
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROGRAM=${1:-${CARGO_TARGET_DIR:-$ROOT/target}/debug/app-ui}
WORK=$(mktemp -d)
PID=""
FAILED=0

cleanup() {
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    hdiutil detach /Volumes/FigIslands >/dev/null 2>&1 || true
    rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "ok     $1"; }
fail() { echo "FAILED $1"; FAILED=1; }
shown() { grep -c "islands: $1 shown" "$WORK/err.log" || true; }

mkdir -p "$WORK/home" "$WORK/downloads"
cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "islands": { "enabled": true, "caps_lock": true, "power": true, "drives": true, "downloads": true, "privacy": true } }
JSON

FIGGHOST_HOME="$WORK/home" FIGGHOST_DOWNLOADS="$WORK/downloads" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_TRACE=1 \
    "$PROGRAM" --background 2> "$WORK/err.log" &
PID=$!
sleep 4
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the islands on"; else fail "the program exited"; cat "$WORK/err.log"; exit 1; fi

# A browser writes name.crdownload and renames it when it is done.
echo "still arriving" > "$WORK/downloads/movie.mp4.crdownload"
sleep 5
if [ "$(shown island-download)" -eq 0 ]; then pass "a file that is still being written is not a download"; else fail "a partial file was announced"; fi
mv "$WORK/downloads/movie.mp4.crdownload" "$WORK/downloads/movie.mp4"
sleep 6
if [ "$(shown island-download)" -ge 1 ]; then pass "a finished download puts an island on the notch"; else fail "no island for a finished download"; fi

# A disk image is a drive that can be ejected.
hdiutil create -size 8m -fs HFS+ -volname FigIslands "$WORK/drive.dmg" >/dev/null 2>&1
hdiutil attach "$WORK/drive.dmg" >/dev/null 2>&1
sleep 3
if [ "$(shown island-drive)" -ge 1 ]; then pass "a drive that is mounted puts an island on the notch"; else fail "no island for a mounted drive"; fi
BEFORE=$(shown island-drive)
hdiutil detach /Volumes/FigIslands >/dev/null 2>&1
sleep 3
if [ "$(shown island-drive)" -gt "$BEFORE" ]; then pass "a drive that is unmounted puts an island on the notch"; else fail "no island for an unmounted drive"; fi

if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; FAILED=1; fi
exit "$FAILED"
