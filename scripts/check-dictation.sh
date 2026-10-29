#!/bin/sh
# Starts the real program on a private instance with Dictation off, then on, and checks through its trace that an off
# feature does nothing, that an on one registers its shortcut, and that pressing it (started by
# FIGGHOST_DICTATION_START, since a global shortcut cannot be pressed from here) shows the panel with what the program
# needs: a copy of the program outside its bundle has no words for the Speech Recognition dialog, so it is never
# asked and says so instead of being ended by the system. The program must neither die nor panic. Nothing of the
# real settings is touched, and the microphone is never opened.
#
# Usage: scripts/check-dictation.sh [program]    (default: target/debug/app-ui)
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

# Starts the program with the Dictation settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "dictation": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_TRACE=1 FIGGHOST_DICTATION_START=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

run '{ "enabled": false }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with Dictation off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "dictation:" "$WORK/off.log"; then fail "an off feature did something"; grep "dictation:" "$WORK/off.log"; else pass "an off feature does nothing"; fi
stop

# A silence that is not offered and a key that does not exist must not make the file unreadable.
run '{ "enabled": true, "language": "vi", "silence_seconds": 7, "future_key": 1 }' on
sleep 6
if grep -q "dictation: cannot register" "$WORK/on.log"; then fail "the shortcut was not registered"; else pass "the shortcut is registered"; fi
if grep -q "dictation: a note with" "$WORK/on.log" || grep -q "dictation: listening in vi" "$WORK/on.log"; then
    pass "the shortcut brings the panel ($(grep -o 'dictation: [a-z].*' "$WORK/on.log" | head -1))"
else
    fail "the shortcut brought nothing"; grep "dictation" "$WORK/on.log" || true
fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running after the shortcut"; else fail "the program died after the shortcut"; tail -20 "$WORK/on.log"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
