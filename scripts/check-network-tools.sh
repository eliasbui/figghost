#!/bin/sh
# Starts the real program on a private instance with the features of the plan of the Network page and the tools all
# on (network history, the alert of a sudden upload, the notices of the connection, save data with a quota and a wire
# to Block, Rewrite, the command palette, the mirror, Commands, the unread count and the speed of videos), and checks that
# it stays up and does not panic, and that the history of the network is written to its file while it runs. Nothing of
# the real settings is touched, the system's proxy settings are not changed (the wires are only kept), the camera is not
# opened and nothing is sent to a model.
#
# Usage: scripts/check-network-tools.sh [program]    (default: target/debug/app-ui)
#        The history is written about once a minute, so this takes about 75 seconds.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROGRAM=${1:-${CARGO_TARGET_DIR:-$ROOT/target}/debug/app-ui}
WORK=$(mktemp -d)
PID=""
FAILED=0

cleanup() {
    if [ -n "$PID" ]; then kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; fi
    rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "ok     $1"; }
fail() { echo "FAILED $1"; FAILED=1; }

mkdir -p "$WORK/home"
cat > "$WORK/home/config.json" <<'JSON'
{ "show_settings_on_launch": false,
  "network": { "enabled": true, "control": false, "history": true, "history_seconds": 2, "anomaly": true, "watch": true,
    "metered": { "enabled": true, "auto": true, "allow": ["Mail"], "cap_mb": 100, "period": "day" },
    "wires": [ { "app": "Steam", "target": "block", "priority": "low", "quota_mb": 5, "quota_period": "day" } ] },
  "rewrite": { "enabled": true }, "palette": { "enabled": true }, "mirror": { "enabled": true },
  "commands": { "enabled": true, "items": [ { "name": "Echo", "command": "echo hi" } ] },
  "notifications": { "enabled": false, "badge": true },
  "browser": { "enabled": true, "speed_control": true, "speed": 1.5 } }
JSON

FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_TRACE=1 "$PROGRAM" --background 2> "$WORK/run.log" &
PID=$!

# Something to count: a download through a connection of this Mac, so that nettop has a socket that moves.
sleep 3
curl -s --max-time 20 -o /dev/null https://www.apple.com/ 2>/dev/null || true
sleep 70

if kill -0 "$PID" 2>/dev/null; then pass "the program runs with every feature of the plan on"; else fail "the program exited"; cat "$WORK/run.log"; exit 1; fi
if grep -qi "panicked" "$WORK/run.log"; then fail "something panicked"; grep -i -A3 panicked "$WORK/run.log"; else pass "nothing panicked"; fi
if grep -q "cannot register" "$WORK/run.log"; then fail "a shortcut was not registered"; grep "cannot register" "$WORK/run.log"; else pass "every shortcut is registered"; fi
if [ -s "$WORK/home/network-history.txt" ] && head -1 "$WORK/home/network-history.txt" | grep -q "^figghost-network-history 1$"; then
    pass "the history of the network was written"
else
    fail "the history of the network was not written"
fi
if [ -s "$WORK/home/network-history.txt" ] && awk -F'\t' 'NR > 1 && NF != 5 { bad = 1 } END { exit bad }' "$WORK/home/network-history.txt"; then
    pass "every line of it has its five fields"
else
    fail "a line of the history is of the wrong shape"
fi
stop() { kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; PID=""; }
stop

if [ "$FAILED" -ne 0 ]; then echo "check failed"; exit 1; fi
echo "all checks passed"
