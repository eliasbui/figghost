#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Devices tab of the notch does:
# with the feature off its tab is not in the strip and nothing is asked of the system; with it on it asks once when the
# tab opens, reads the answer, finds as many connected Bluetooth devices as `system_profiler` itself lists, and neither
# asks nor keeps its half-minute timer once the tab has closed. The notch is placed outside every screen and the
# settings window stays closed. Nothing of the real settings is touched.
#
# Usage: scripts/check-devices.sh [program]    (default: target/debug/app-ui)
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
count() { grep -c "devices: $1" "$2" || true; }

# Starts the program with the Devices settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "devices": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=devices FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

# How many connected devices the system itself lists, asked of it with a time limit of its own; "unknown" without an
# answer, and then that comparison is skipped.
EXPECTED=$(/usr/bin/python3 -I -c '
import json, subprocess
try:
    out = subprocess.run(["/usr/sbin/system_profiler", "SPBluetoothDataType", "-json"], capture_output=True, text=True, timeout=25).stdout
    data = json.loads(out)
    print(sum(len(device) for section in data["SPBluetoothDataType"] for device in section.get("device_connected", [])))
except Exception:
    print("unknown")
')

# Off: the tab is not in the strip, so it cannot be opened, and nothing is asked.
run '{ "enabled": false }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Devices tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab devices is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'reading' "$WORK/off.log")" -eq 0 ]; then pass "an off tab asks the system nothing"; else fail "an off tab asked the system"; fi
stop

# On: one look when the tab opens, and none after it closes (it closes about seven seconds after it opens, and a timer
# that was kept would look again at thirty seconds).
run '{ "enabled": true }' on
sleep 6
if [ "$(count 'reading' "$WORK/on.log")" -eq 1 ]; then pass "an open tab asks once"; else fail "the tab asked $(count 'reading' "$WORK/on.log") times while it was open"; fi
ARRIVED=$(grep "devices: [0-9]* connected" "$WORK/on.log" | head -1 || true)
if [ -n "$ARRIVED" ]; then pass "the answer was read: ${ARRIVED#*devices: }"; else fail "no answer was read"; grep "devices:" "$WORK/on.log" || true; fi
if [ -n "$ARRIVED" ] && [ "$EXPECTED" != "unknown" ]; then
    FOUND=$(echo "$ARRIVED" | sed -E 's/.*devices: ([0-9]+) connected.*/\1/')
    if [ "$FOUND" = "$EXPECTED" ]; then pass "it found the $EXPECTED connected devices that system_profiler lists"; else fail "it found $FOUND connected devices and system_profiler lists $EXPECTED"; fi
else
    echo "note   the number of devices was not compared (no answer from system_profiler in time)"
fi
sleep 5
if [ "$(count 'closed' "$WORK/on.log")" -ge 1 ]; then pass "the tab closed"; else fail "the tab never closed"; fi
sleep 26
if [ "$(count 'reading' "$WORK/on.log")" -eq 1 ]; then pass "nothing is asked while the tab is closed"; else fail "it asked again after the tab closed"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
