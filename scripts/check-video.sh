#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Video tab of the notch does:
# with the feature off its tab is not in the strip and nothing plays; with it on and a video on this Mac it plays
# when the tab opens and stops when it closes; with an address that cannot be opened it says so. The notch is placed
# outside every screen and the settings window stays closed. Nothing of the real settings is touched.
#
# Usage: scripts/check-video.sh [program]    (default: target/debug/app-ui)
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROGRAM=${1:-${CARGO_TARGET_DIR:-$ROOT/target}/debug/app-ui}
CLIP="/System/Library/Wallpapers/.default/Golden Gate.mov"
WORK=$(mktemp -d)
PID=""
FAILED=0

cleanup() {
    if [ -n "$PID" ]; then
        kill "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "ok     $1"; }
fail() { echo "FAILED $1"; FAILED=1; }
count() { grep -c "video: $1" "$2" || true; }

# Starts the program with the Video settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "video": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=video FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

run '{ "enabled": false }' off
sleep 4
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Video tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab video is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'playing' "$WORK/off.log")" -eq 0 ]; then pass "an off tab plays nothing"; else fail "an off tab played"; fi
stop

run '{ "enabled": true, "url": "" }' empty
sleep 4
if [ "$(count 'playing' "$WORK/empty.log")" -eq 0 ]; then pass "an open tab with no address plays nothing"; else fail "it played with no address"; fi
stop

if [ -f "$CLIP" ]; then
    run "{ \"enabled\": true, \"url\": \"file://$(printf '%s' "$CLIP" | sed 's/ /%20/g')\" }" on
    sleep 5
    if [ "$(count 'playing' "$WORK/on.log")" -eq 1 ]; then pass "an open tab plays the video once"; else fail "the tab did not play once"; fi
    if [ "$(count 'failed' "$WORK/on.log")" -eq 0 ]; then pass "the video did not fail"; else fail "the video failed"; fi
    sleep 5
    if [ "$(count 'closed' "$WORK/on.log")" -ge 1 ]; then pass "the tab closed and the player was dropped"; else fail "the tab never closed"; fi
    stop
else
    echo "note   $CLIP is not on this Mac, so playing a video was not tried"
fi

run '{ "enabled": true, "url": "file:///nonexistent/none.mov" }' broken
sleep 5
if [ "$(count 'failed' "$WORK/broken.log")" -ge 1 ]; then pass "an address that cannot be opened is reported"; else fail "a broken address was not reported"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK"/*.log; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK"/*.log; fi
stop
exit "$FAILED"
