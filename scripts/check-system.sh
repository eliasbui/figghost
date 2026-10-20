#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the System tab of the notch
# does: with the feature off its tab is not in the strip and nothing is read, and with it on it reads while the tab is open
# and not once after it closes. The notch is placed outside every screen and the settings window stays closed.
# Nothing of the real settings is touched.
#
# Usage: scripts/check-system.sh [program]    (default: target/debug/app-ui)
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
count() { grep -c "system: $1" "$2" || true; }

# Starts the program with the System feature settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "system": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=system FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

# Off: the tab is not in the strip, so it cannot be opened, and nothing is read.
run '{ "enabled": false }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the System tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab system is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'read$' "$WORK/off.log")" -eq 0 ]; then pass "an off tab reads nothing"; else fail "an off tab read the system"; fi
stop

# On: reads while the tab shows, one reading a second, and stops when it closes.
run '{ "enabled": true, "seconds": 1 }' on
sleep 5
if [ "$(count 'reading starts' "$WORK/on.log")" -ge 1 ]; then pass "an open tab starts reading"; else fail "the tab never started reading"; fi
READS=$(count 'read$' "$WORK/on.log")
if [ "$READS" -ge 3 ]; then pass "it reads about once a second ($READS readings so far)"; else fail "too few readings: $READS"; fi
sleep 5
if [ "$(count 'reading stops' "$WORK/on.log")" -ge 1 ]; then pass "a closed tab stops reading"; else fail "the readings never stopped"; fi
AFTER=$(count 'read$' "$WORK/on.log")
sleep 4
if [ "$(count 'read$' "$WORK/on.log")" -eq "$AFTER" ]; then pass "nothing is read while the tab is closed"; else fail "it kept reading after the tab closed"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
