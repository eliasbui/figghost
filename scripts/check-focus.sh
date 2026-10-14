#!/bin/sh
# Regression check for the non-activating requirement: launching app-ui must not change the frontmost app.
# Run it from a terminal while that terminal is the frontmost app. macOS only lets a process take focus
# at launch when the frontmost app is its ancestor, so with any other app in front a buggy build passes too.
set -eu

BIN=${1:-target/release/app-ui}
RUNS=${2:-5}
[ -x "$BIN" ] || { echo "build first: cargo build --release -p app-ui ($BIN not found)"; exit 2; }

# Prints "<frontmost app>|<its pid>|<number of on-screen windows owned by app-ui>".
probe() {
  osascript -l JavaScript -e '
    ObjC.import("AppKit"); ObjC.import("CoreGraphics");
    const app = $.NSWorkspace.sharedWorkspace.frontmostApplication;
    const all = ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0)));
    ObjC.unwrap(app.localizedName) + "|" + app.processIdentifier + "|" + all.filter(w => w.kCGWindowOwnerName === "app-ui").length'
}

is_ancestor() {
  p=$$
  while [ "$p" -gt 1 ]; do
    [ "$p" = "$1" ] && return 0
    p=$(ps -o ppid= -p "$p" | tr -d ' ')
    [ -n "$p" ] || break
  done
  return 1
}

first=$(probe)
before=${first%%|*}
rest=${first#*|}
before_pid=${rest%%|*}
# loginwindow stays frontmost on a locked screen, so every run would pass for the wrong reason.
[ "$before" != "loginwindow" ] || { echo "screen is locked: result would be meaningless"; exit 2; }
is_ancestor "$before_pid" || {
  echo "frontmost app '$before' did not launch this script: the check cannot fail. Bring your terminal to the front and rerun."
  exit 2
}

run=1
while [ "$run" -le "$RUNS" ]; do
  "$BIN" >/dev/null 2>&1 &
  pid=$!
  sleep 3
  now=$(probe)
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  front=${now%%|*}
  windows=${now##*|}
  # A crashed app would also leave the frontmost app unchanged, so require the panel to be on screen.
  [ "$windows" -ge 1 ] || { echo "FAIL run $run: app-ui has no window on screen"; exit 1; }
  [ "$front" = "$before" ] || { echo "FAIL run $run: frontmost changed from '$before' to '$front'"; exit 1; }
  run=$((run + 1))
done
echo "OK: frontmost stayed '$before' across $RUNS launches, panel visible each time"
