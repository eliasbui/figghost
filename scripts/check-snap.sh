#!/bin/sh
# The checks of window snapping that need no screen and post no input: the real controller, zones panel,
# footprint and notch on the pretend screen far outside every real one (snap_check), the Accessibility lookup
# against one point of the real screen, printing only yes or no (grab_check, which reports a skip when the desktop
# has nothing to find or Accessibility is not granted to the terminal), and the drawings of the zones and the
# footprint (render_snap). Run from the repository root.
set -eu

TARGET=${CARGO_TARGET_DIR:-target}
cargo build -p macos-sys --example grab_check --example render_snap -p app-ui --example snap_check
FIGGHOST_NOTCH_OFFSCREEN=1 "$TARGET/debug/examples/snap_check" "${1:-/tmp/snap-live}"
"$TARGET/debug/examples/grab_check" || [ $? -eq 3 ]
"$TARGET/debug/examples/render_snap" "${1:-/tmp/snap-live}"
