#!/bin/sh
# Starts the real program on a private instance with the Launcher tab pinned to an app, a folder, a Shortcut and
# two pins that cannot work (an app that is not installed, a folder that is not there), opens the notch on the tab
# and checks, through its trace, that the tab is drawn with every pin and the program neither dies nor panics, and
# that with the feature off nothing is drawn. Nothing is pressed, so nothing is launched. The notch is placed outside
# every screen and the settings window stays closed. Nothing of the real settings is touched.
#
# Usage: scripts/check-launcher.sh [program]    (default: target/debug/app-ui)
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

# Starts the program with the Launcher settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "launcher": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=launcher FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

ITEMS='[
  { "kind": "app", "target": "com.apple.finder", "name": "Finder" },
  { "kind": "folder", "target": "/Users", "name": "Users" },
  { "kind": "shortcut", "target": "Morning routine" },
  { "kind": "app", "target": "com.example.NotInstalled", "name": "Gone" },
  { "kind": "folder", "target": "/this/folder/is/not/there" },
  { "kind": "robot", "target": "dropped" }
]'

run "{ \"enabled\": false, \"items\": $ITEMS }" off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Launcher tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if [ "$(grep -c 'launcher: showing' "$WORK/off.log" || true)" -eq 0 ]; then pass "an off tab draws no tiles"; else fail "an off tab drew tiles"; fi
stop

run "{ \"enabled\": true, \"items\": $ITEMS }" on
sleep 6
if grep -q "launcher: showing 5 tiles" "$WORK/on.log"; then pass "an open tab draws the five pins that can be read and drops the sixth"; else fail "the tiles were not drawn"; grep launcher "$WORK/on.log" || true; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running with pins that cannot work"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop

run '{ "enabled": true }' empty
sleep 5
if grep -q "launcher: showing" "$WORK/empty.log"; then fail "an empty tab drew tiles"; else pass "an empty tab draws no tiles and says nothing is pinned"; fi
stop
exit "$FAILED"
