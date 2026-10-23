#!/bin/sh
# Checks the video download feature: its tests against a stand-in for yt-dlp, the real program started on a private
# instance with the feature on (the notch is placed outside every screen and the settings window stays closed), and that
# the live islands say nothing of the folder yt-dlp keeps what it has not finished in. Nothing of the real settings or the
# real Downloads folder is touched, and no key or mouse input is sent.
#
# With a link as the second argument it also downloads that link with the installed yt-dlp into a folder of its own, checks
# that the file opens with a picture and a sound (a music only link, with one sound), and removes the file afterwards.
#
# Usage: scripts/check-download.sh [program] [link]    (program default: target/debug/app-ui)
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROGRAM=${1:-${CARGO_TARGET_DIR:-$ROOT/target}/debug/app-ui}
LINK=${2:-}
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

cd "$ROOT"
if cargo test -p app-ui features::download 2>&1 | tee "$WORK/test.log" | grep -q "test result: ok"; then
    pass "the unit tests and the lifecycle tests against the stand-in pass"
else
    fail "the tests failed"
    tail -30 "$WORK/test.log"
fi

if [ -n "$LINK" ]; then
    if FIGGHOST_TEST_URL="$LINK" cargo test -p app-ui real_download -- --ignored --nocapture 2>&1 | tee "$WORK/real.log" | grep -q "test result: ok"; then
        pass "a real download with the installed yt-dlp saved a file"
    else
        fail "the real download failed"
    fi
    grep "^result\|^title\|^left\|^ffmpeg" "$WORK/real.log" || true
fi

cargo build -p app-ui 2>&1 | tail -1
mkdir -p "$WORK/home" "$WORK/downloads"
cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "download": { "enabled": true }, "islands": { "enabled": true, "downloads": true } }
JSON
FIGGHOST_HOME="$WORK/home" FIGGHOST_DOWNLOADS="$WORK/downloads" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_TRACE=1 \
    "$PROGRAM" --background 2> "$WORK/err.log" &
PID=$!
sleep 4
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with the feature on"; else fail "the program exited"; cat "$WORK/err.log"; exit 1; fi

# With the feature on and nothing downloaded, nothing wakes: the timer of the feature comes with the first download.
if grep -q 'download: timer' "$WORK/err.log"; then fail "the feature started its timer with nothing to download"; else pass "nothing is woken while the feature is on and idle"; fi

# What yt-dlp has not finished lives in a hidden folder of the folder downloads go to, which the islands never list. A
# finished file of the folder is announced, and that is the control: where it is not (a session with no screen to place
# the notch on) the check says it cannot tell instead of passing for nothing.
shown() { grep -c 'islands: island-download shown' "$WORK/err.log" || true; }
mkdir "$WORK/downloads/.figghost-download-1-1"
echo "half" > "$WORK/downloads/.figghost-download-1-1/movie.f137.mp4.part"
echo "half" > "$WORK/downloads/.figghost-download-1-1/movie.f140.m4a"
sleep 5
NOISE=$(shown)
echo "whole" > "$WORK/downloads/movie.mp4"
sleep 6
if [ "$(shown)" -eq 0 ]; then
    echo "skip   the islands announced nothing here, not even a finished file, so the folder of unfinished files cannot be checked"
elif [ "$NOISE" -eq 0 ]; then
    pass "what yt-dlp has not finished is not announced, and a finished file is"
else
    fail "an unfinished file was announced"
fi

if kill -0 "$PID" 2>/dev/null; then pass "the program is still running"; else fail "the program died"; fi
exit "$FAILED"
