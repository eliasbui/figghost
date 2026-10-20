#!/bin/sh
# Starts the real program on a private instance and checks, through its trace, what the Pull requests tab does: with
# the feature off, or with no service chosen, it runs nothing; with a service chosen it asks the signed-in command-line
# tool for the review requests once soon after the app starts and reads the answer. It prints how many requests and how
# many services answered, never what they are. The notch is placed outside every screen and the settings window stays
# closed. Nothing of the real settings is touched. The online checks are skipped, with a note, when neither `gh` nor
# `glab` is installed and signed in.
#
# Usage: scripts/check-prs.sh [program]    (default: target/debug/app-ui)
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

# Starts the program with the Pull requests settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "prs": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=prs FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

run '{ "enabled": false }' off
sleep 9
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Pull requests tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if ! grep -q "prs: asking" "$WORK/off.log"; then pass "an off tab asks nothing"; else fail "an off tab ran a tool"; fi
stop

run '{ "enabled": true, "github": false, "gitlab": false }' none
sleep 9
if ! grep -q "prs: asking" "$WORK/none.log"; then pass "with no service chosen nothing is asked"; else fail "it ran a tool with no service chosen"; fi
stop

HAVE=""
for tool in gh glab; do
    for dir in /opt/homebrew/bin /usr/local/bin /usr/bin; do
        if [ -x "$dir/$tool" ] && "$dir/$tool" auth status >/dev/null 2>&1; then HAVE="$HAVE $tool"; break; fi
    done
done
if [ -z "$HAVE" ]; then
    echo "skip   neither gh nor glab is installed and signed in, so the online checks were not run"
    exit "$FAILED"
fi

run '{ "enabled": true, "github": true, "gitlab": true }' on
sleep 40
if [ "$(grep -c 'prs: asking' "$WORK/on.log" || true)" -eq 1 ]; then pass "an open feature asks once after the app starts"; else fail "it did not ask exactly once"; grep prs "$WORK/on.log" || true; fi
LINE=$(grep "prs: [0-9]* requests" "$WORK/on.log" | head -1 || true)
if [ -n "$LINE" ]; then pass "the answer was read: ${LINE#* }"; else fail "no answer was read"; grep prs "$WORK/on.log" || true; fi
WANT=$(echo "$HAVE" | wc -w | tr -d ' ')
if echo "$LINE" | grep -q "$WANT sources answered"; then pass "every tool that is signed in answered ($WANT)"; else fail "a signed-in tool did not answer"; echo "$LINE"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log"; then fail "the program panicked"; fi
stop
exit "$FAILED"
