#!/bin/sh
# Starts the real program on a private instance with the Launcher tab pinned to an app, a folder, a Shortcut and
# two pins that cannot work (an app that is not installed, a folder that is not there), written the way files were
# before there were groups, opens the notch on the tab and checks, through its trace, that the tab is drawn with every
# pin and the program neither dies nor panics, and that with the feature off nothing is drawn. Then it starts it with
# groups, a file, a website and the dock on: the dock is put at its edge with the tiles of its group, hidden until the
# pointer comes when asked, and the dock off puts nothing on the screen. Nothing is pressed, so nothing is launched.
# The notch is placed outside every screen and the settings window stays closed. Nothing of the real settings is
# touched.
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
if grep -q "launcher: the dock" "$WORK/empty.log"; then fail "the dock came while it is off"; else pass "the dock stays away while it is off"; fi
stop

SETS='[
  { "name": "", "items": [ { "kind": "app", "target": "com.apple.finder" }, { "kind": "folder", "target": "/Users" } ] },
  { "name": "Web", "items": [
    { "kind": "url", "target": "https://github.com" },
    { "kind": "url", "target": "mailto:me@example.com" },
    { "kind": "file", "target": "/etc/hosts" },
    { "kind": "url", "target": "javascript:alert(1)" }
  ] }
]'
run "{ \"enabled\": true, \"groups\": $SETS, \"dock\": true, \"dock_edge\": \"right\", \"dock_group\": 1, \"dock_autohide\": false }" dock
sleep 6
if grep -q "launcher: showing 2 tiles" "$WORK/dock.log"; then pass "the tab shows the first group"; else fail "the tab did not show the first group"; grep launcher "$WORK/dock.log" || true; fi
if grep -q "launcher: the dock shows 3 tiles at the Right edge$" "$WORK/dock.log"; then pass "the dock shows the three good pins of its group at its edge and drops the bad address"; else fail "the dock was not shown"; grep launcher "$WORK/dock.log" || true; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running with the dock"; else fail "the program died with the dock"; fi
stop

run "{ \"enabled\": false, \"groups\": $SETS, \"dock\": true }" hidden
sleep 5
if grep -q "launcher: the dock shows 2 tiles at the Left edge, hidden until the pointer comes" "$WORK/hidden.log"; then pass "a dock that hides waits for the pointer, with the tab off"; else fail "the hidden dock was not set up"; grep launcher "$WORK/hidden.log" || true; fi
if grep -q "panicked" "$WORK/dock.log" "$WORK/hidden.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/dock.log" "$WORK/hidden.log"; fi
stop
exit "$FAILED"
