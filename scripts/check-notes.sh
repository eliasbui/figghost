#!/bin/sh
# Starts the real program on a private instance with three notes in its notes folder and checks, through its
# trace, what the Notes tab does: with the feature off its tab is not in the strip and nothing is read, and with it on it opens
# on the notes that are there, and a blank note that was started and left leaves no file. The notch is placed outside
# every screen and the settings window stays closed. Nothing of the real settings or notes is touched.
#
# Usage: scripts/check-notes.sh [program]    (default: target/debug/app-ui)
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
count() { grep -c "notes: $1" "$2" || true; }

# Starts the program with the Notes settings in $1 and leaves its trace in $WORK/$2.log.
run() {
    mkdir -p "$WORK/home/notes"
    printf 'Trip plan\nbook the train\n' > "$WORK/home/notes/note-1760000000001.md"
    printf 'Groceries\nmilk, eggs\n' > "$WORK/home/notes/note-1760000000002.md"
    printf 'Ideas\n' > "$WORK/home/notes/note-1760000000003.md"
    printf 'not a note\n' > "$WORK/home/notes/readme.txt"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "notes": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_NOTCH_OPEN=notes FIGGHOST_TRACE=1 \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

# Off: the tab is not in the strip, so it cannot be opened, and the notes are not read.
run '{ "enabled": false }' off
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the Notes tab off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "FIGGHOST_NOTCH_OPEN: the tab notes is not in the strip" "$WORK/off.log"; then pass "an off tab is not in the strip, so it cannot be opened"; else fail "an off tab was in the strip"; fi
if [ "$(count 'opened' "$WORK/off.log")" -eq 0 ]; then pass "an off tab reads no notes"; else fail "an off tab opened the notes"; fi
stop

# On, starting a blank note each time: it opens on the notes that are there, and the blank one leaves no file.
run '{ "enabled": true, "new_on_open": true }' on
sleep 10
if [ "$(count 'opened with 3 notes' "$WORK/on.log")" -ge 1 ]; then pass "an open tab finds the three notes and nothing else"; else fail "the notes were not found"; grep notes "$WORK/on.log" || true; fi
if [ "$(ls "$WORK/home/notes" | wc -l | tr -d ' ')" -eq 4 ]; then pass "the blank note left no file and nothing was changed"; else fail "the folder changed"; ls "$WORK/home/notes"; fi
if [ "$(cat "$WORK/home/notes/note-1760000000001.md")" = "$(printf 'Trip plan\nbook the train')" ]; then pass "a note that was only read is untouched"; else fail "a note was rewritten"; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
if grep -q "panicked" "$WORK/on.log" "$WORK/off.log"; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK/on.log" "$WORK/off.log"; fi
stop
exit "$FAILED"
