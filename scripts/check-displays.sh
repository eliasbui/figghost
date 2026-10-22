#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Displays tab of the notch does:
# with the feature off its tab is not in the strip and no display is asked anything; with it on it looks for the
# displays once when the tab opens and reads how many answer, and looks no more once the tab has closed. Nothing is
# set on any display: only the brightness and contrast are asked. The notch is placed outside every screen and the
# settings window stays closed. Nothing of the real settings is touched.
#
# Usage: scripts/check-displays.sh [program]    (default: target/debug/app-ui)
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
    rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "ok     $1"; }
fail() { echo "FAILED $1"; FAILED=1; }
count() { grep -c "displays: $1" "$2" || true; }

# Starts the program with the Displays settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "displays": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=displays FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

# Off: the tab is not in the strip, so it cannot be opened, and no display is asked anything.
run '{ "enabled": false }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Displays tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab displays is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'looking' "$WORK/off.log")" -eq 0 ]; then pass "an off tab asks no display anything"; else fail "an off tab looked for displays"; fi
stop

# On: one look when the tab opens, and none after it closes (about seven seconds after it opens).
run '{ "enabled": true }' on
sleep 6
if [ "$(count 'looking' "$WORK/on.log")" -eq 1 ]; then pass "an open tab looks once"; else fail "the tab looked $(count 'looking' "$WORK/on.log") times while it was open"; fi
ANSWERED=$(grep "displays: [0-9]* answer" "$WORK/on.log" | head -1 || true)
if [ -n "$ANSWERED" ]; then pass "the look ended: ${ANSWERED#*displays: }"; else fail "the look never ended"; grep "displays:" "$WORK/on.log" || true; fi
sleep 10
if [ "$(count 'looking' "$WORK/on.log")" -eq 1 ]; then pass "nothing is looked for once the tab has closed"; else fail "it looked again after the tab closed"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
