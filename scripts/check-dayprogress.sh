#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Day progress tab of the notch
# does: with the feature off its tab is not in the strip and nothing is read; with it on it reads the day once when the tab opens
# (or says that the calendar is not allowed), and it neither reads nor keeps its half-minute timer once the tab has
# closed. The notch is placed outside every screen and the settings window stays closed. Nothing of the real settings
# is touched.
#
# Usage: scripts/check-dayprogress.sh [program]    (default: target/debug/app-ui)
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
count() { grep -c "dayprogress: $1" "$2" || true; }

# Starts the program with the Day progress settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "dayprogress": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=dayprogress FIGGHOST_TRACE=1 \
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
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Day progress tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab dayprogress is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'read' "$WORK/off.log")" -eq 0 ]; then pass "an off tab reads nothing"; else fail "an off tab read the calendar"; fi
stop

# On: one look when the tab opens, and none after it closes (it closes about seven seconds after it opens, and a timer
# that was kept would look again at thirty seconds).
run '{ "enabled": true }' on
sleep 6
LOOKS=$(( $(count 'read' "$WORK/on.log") + $(count 'no access' "$WORK/on.log") ))
if [ "$LOOKS" -eq 1 ]; then pass "an open tab looks at the day once"; else fail "the tab looked $LOOKS times while it was open"; fi
if [ "$(count 'no access' "$WORK/on.log")" -eq 1 ]; then echo "note   the calendar is not allowed for this program, so the reading itself was not tried"; fi
sleep 5
if [ "$(count 'closed' "$WORK/on.log")" -ge 1 ]; then pass "the tab closed"; else fail "the tab never closed"; fi
sleep 26
LOOKS=$(( $(count 'read' "$WORK/on.log") + $(count 'no access' "$WORK/on.log") ))
if [ "$LOOKS" -eq 1 ]; then pass "nothing is looked at while the tab is closed"; else fail "it looked again after the tab closed ($LOOKS looks)"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
