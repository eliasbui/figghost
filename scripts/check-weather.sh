#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Weather tab does: off, or on
# with no city, it asks nothing of the internet; on with a city, it asks Open-Meteo once when the tab opens and
# shows the answer. The notch is placed outside every screen and the settings window stays closed. Nothing of the
# real settings is touched. The last check needs a connection and is skipped, with a note, without one.
#
# Usage: scripts/check-weather.sh [program]    (default: target/debug/app-ui)
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
count() { grep -c "weather: $1" "$2" || true; }

# Starts the program with the Weather settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "weather": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=weather FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

run '{ "enabled": false, "place": { "name": "Hanoi", "latitude": 21.0245, "longitude": 105.84117 } }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Weather tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab weather is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'asking' "$WORK/off.log")" -eq 0 ]; then pass "an off tab asks the internet nothing"; else fail "an off tab went online"; fi
stop

run '{ "enabled": true }' nocity
sleep 5
if [ "$(count 'nothing to fetch' "$WORK/nocity.log")" -ge 1 ] && [ "$(count 'asking' "$WORK/nocity.log")" -eq 0 ]; then pass "a tab with no city asks the internet nothing"; else fail "a tab with no city went online"; fi
stop

if ! curl -s -m 8 -o /dev/null "https://api.open-meteo.com/v1/forecast?latitude=0&longitude=0&current=temperature_2m"; then
    echo "skip   no connection to Open-Meteo, so the online check was not run"
    exit "$FAILED"
fi
run '{ "enabled": true, "place": { "name": "Hanoi", "country": "Vietnam", "latitude": 21.0245, "longitude": 105.84117 } }' on
sleep 12
if [ "$(count 'asking for the forecast' "$WORK/on.log")" -eq 1 ]; then pass "an open tab asks once"; else fail "the tab did not ask exactly once"; grep weather "$WORK/on.log" || true; fi
if [ "$(count 'forecast arrived' "$WORK/on.log")" -eq 1 ]; then pass "the forecast arrived and was read"; else fail "no forecast arrived"; grep weather "$WORK/on.log" || true; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log" "$WORK/nocity.log"; then fail "the program panicked"; fi
stop
exit "$FAILED"
