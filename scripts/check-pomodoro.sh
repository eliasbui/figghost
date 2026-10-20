#!/bin/sh
# Starts the real program on private instances with the Pomodoro tab on (with a shortcut) and off, opens the notch on the
# tab, and checks through its trace and its errors that it starts and opens the tab without trouble, that nothing runs
# until a stage is started, and that the shortcut registers without an error. The notch is placed outside every screen and
# the settings window stays closed. Nothing of the real settings is touched.
#
# Pressing the shortcut is not checked here: key events posted to the system do not reach a global shortcut when no
# keyboard sent them, and the system lets two apps register the same combination, so neither a press nor a clash can be
# shown. Starting and pausing by key, and the countdown in the closed notch, are left to the logic tests (the stage machine is
# driven through whole rounds by them) and to a person.
#
# Usage: scripts/check-pomodoro.sh [program]    (default: target/debug/app-ui)
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

# Starts the program with the Pomodoro settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "pomodoro": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=pomodoro FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

run '{ "enabled": false, "shortcut": "ctrl+opt+cmd+f9" }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Pomodoro tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
stop

run '{ "enabled": true, "shortcut": "ctrl+opt+cmd+f9" }' on
sleep 6
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Pomodoro tab on and a shortcut"; else fail "the program exited"; cat "$WORK/on.log"; exit 1; fi
if ! grep -q "pomodoro: cannot register" "$WORK/on.log"; then pass "the shortcut registered without an error"; else fail "the shortcut could not be registered"; cat "$WORK/on.log"; fi
if ! grep -q "pomodoro: running" "$WORK/on.log"; then pass "nothing runs until a stage is started"; else fail "it started by itself"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop

# A shortcut that cannot be parsed is dropped quietly, never a crash.
run '{ "enabled": true, "shortcut": "nonsense" }' bad
sleep 5
if kill -0 "$PID" 2>/dev/null && ! grep -q "panicked" "$WORK/bad.log"; then pass "a shortcut that is nonsense is read as none"; else fail "a nonsense shortcut hurt the program"; cat "$WORK/bad.log"; fi
stop
exit "$FAILED"
