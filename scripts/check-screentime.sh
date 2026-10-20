#!/bin/sh
# Starts the real program on a private instance and checks what Screen Time does: with the feature off it counts
# nothing and writes nothing; with it on and someone at the Mac (a mouse-move event is posted, to where the pointer
# already is, every two seconds, so nothing moves) it counts the app in front, writes the tally when the tab closes, and
# the file holds whole seconds for an app with a name; with it on and nobody at the Mac it counts nothing. The last check
# is skipped, with a note, when someone has used the Mac in the five minutes before. The notch is placed outside every screen
# and the settings window stays closed. Nothing of the real settings is touched.
#
# Usage: scripts/check-screentime.sh [program]    (default: target/debug/app-ui)
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

# Seconds since the last input of any kind, and a way to make some: a mouse-move event at the pointer's own place.
idle_seconds() {
    /usr/bin/python3 -I - <<'PY'
import ctypes, ctypes.util
cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
cg.CGEventSourceSecondsSinceLastEventType.restype = ctypes.c_double
cg.CGEventSourceSecondsSinceLastEventType.argtypes = [ctypes.c_int32, ctypes.c_uint32]
print(int(cg.CGEventSourceSecondsSinceLastEventType(0, 0xFFFFFFFF)))
PY
}
touch_input() {
    /usr/bin/python3 -I - <<'PY'
import ctypes, ctypes.util
cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))
class P(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]
cg.CGEventCreate.restype = ctypes.c_void_p
cg.CGEventCreate.argtypes = [ctypes.c_void_p]
cg.CGEventGetLocation.restype = P
cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, P, ctypes.c_uint32]
cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
here = cg.CGEventGetLocation(cg.CGEventCreate(None))
cg.CGEventPost(0, cg.CGEventCreateMouseEvent(None, 5, here, 0))
PY
}

# Starts the program with the Screen Time settings in $1 and leaves its trace in $WORK/$2.log; the tally goes to $WORK/home.
run() {
    rm -rf "$WORK/home"
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "screentime": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=screentime FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

# Runs for $1 seconds, making input every two seconds when $2 is "busy".
wait_for() {
    left=$1
    while [ "$left" -gt 0 ]; do
        if [ "${2:-}" = "busy" ]; then touch_input; fi
        sleep 2
        left=$((left - 2))
    done
}

run '{ "enabled": false }' off
wait_for 12 busy
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with Screen Time off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if [ ! -e "$WORK/home/screentime.json" ] && ! grep -q "screentime: counting" "$WORK/off.log"; then pass "an off feature counts nothing and writes nothing"; else fail "an off feature counted"; fi
stop

run '{ "enabled": true, "idle_minutes": 5 }' busy
wait_for 14 busy
if grep -q "screentime: counting" "$WORK/busy.log"; then pass "an on feature starts counting"; else fail "it never started"; fi
if [ "$(grep -c 'screentime: counted a look' "$WORK/busy.log" || true)" -ge 1 ]; then pass "with someone at the Mac it counts the app in front"; else fail "nothing was counted with input going on"; grep screentime "$WORK/busy.log" || true; fi
if [ -s "$WORK/home/screentime.json" ] && /usr/bin/python3 -I -c '
import json, sys
t = json.load(open(sys.argv[1]))
days = t["days"]
assert len(days) == 1, days
apps = list(days.values())[0]
assert apps and all(isinstance(s, int) and 0 < s <= 86400 for s in apps.values()), apps
assert all(t["names"].get(a) for a in apps), t
' "$WORK/home/screentime.json"; then pass "the file holds whole seconds for apps that have names"; else fail "the file is missing or wrong"; cat "$WORK/home/screentime.json" 2>/dev/null || true; fi
if grep -q "panicked" "$WORK/busy.log" "$WORK/off.log"; then fail "the program panicked"; fi
stop

# Nobody at the Mac: the shortest idle time is a minute, so wait until a minute and a bit has gone without input. If
# someone really uses the Mac meanwhile the wait is for nothing and the check says so.
IDLE=$(idle_seconds)
if [ "$IDLE" -lt 70 ]; then
    echo "wait   a minute without input is needed for the last check ($IDLE s so far)"
    sleep $((72 - IDLE))
fi
run '{ "enabled": true, "idle_minutes": 1 }' away
wait_for 12 away
if [ "$(idle_seconds)" -ge 60 ]; then
    if [ ! -s "$WORK/home/screentime.json" ] && ! grep -q "counted a look" "$WORK/away.log"; then pass "with nobody at the Mac it counts nothing"; else fail "it counted while nobody was there"; fi
else
    echo "skip   someone used the Mac during the check with nobody there"
fi
stop
exit "$FAILED"
