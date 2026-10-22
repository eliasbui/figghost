#!/bin/sh
# Builds and tests the Windows and Linux beta (crates/desktop) on Linux in Docker, then starts it on a virtual screen,
# copies a text and takes pictures of the window. With --windows it also builds the Windows program with MinGW.
# Output: target/desktop-check/*.png and, with --windows, target/desktop-check/figghost.exe.
#
#   [CHECK_LANG=vi_VN.UTF-8] scripts/check-desktop-linux.sh [--windows]
set -eu
cd "$(dirname "$0")/.."
OUT=target/desktop-check
mkdir -p "$OUT"
IMAGE=figghost-desktop-check

docker build -q -t "$IMAGE" - >/dev/null <<'EOF'
FROM rust:1-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libsqlite3-dev \
      xvfb xclip xdotool imagemagick dbus-x11 fonts-noto-core mingw-w64 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add x86_64-pc-windows-gnu
EOF

WINDOWS=0
[ "${1:-}" = "--windows" ] && WINDOWS=1

docker run --rm \
  -v "$PWD":/src:ro -v figghost-desktop-target:/target -v figghost-desktop-cargo:/usr/local/cargo/registry \
  -v "$PWD/$OUT":/out -e CARGO_TARGET_DIR=/target -e WINDOWS="$WINDOWS" -e CHECK_LANG="${CHECK_LANG:-en_US.UTF-8}" -w /src "$IMAGE" sh -euc '
    cargo test -q -p core-clipboard -p core-agents -p core-notes -p figghost-desktop
    cargo build -q -p figghost-desktop
    if [ "$WINDOWS" = 1 ]; then
      cargo build -q --release -p figghost-desktop --target x86_64-pc-windows-gnu
      cp /target/x86_64-pc-windows-gnu/release/figghost.exe /out/
    fi
    export FIGGHOST_HOME=$(mktemp -d) DISPLAY=:99 WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 LANG=$CHECK_LANG
    Xvfb :99 -screen 0 1100x720x24 >/dev/null 2>&1 &
    sleep 1
    eval "$(dbus-launch --sh-syntax)"
    printf "%s" "https://example.com/figghost-linux-beta" | xclip -selection clipboard
    : >/out/run.log; /target/debug/figghost >/out/app.log 2>&1 &
    sleep 4
    printf "%s" "meeting notes for tuesday" | xclip -selection clipboard
    sleep 2
    import -window root /out/linux-clipboard.png
    # A click on the older item puts it back on the clipboard and closes the window.
    xdotool mousemove 500 305 click 1
    sleep 2
    echo "clipboard after the click: $(xclip -o -selection clipboard)" >>/out/run.log
    # A second start shows the running window and leaves.
    /target/debug/figghost; echo "second start exit: $?" >>/out/run.log
    sleep 3
    import -window root /out/linux-shown-again.png
    # The other pages, from the sidebar.
    for page in notes:107 agents:140 settings:173 about:206; do
      xdotool mousemove 100 "${page#*:}" click 1
      sleep 1
      import -window root "/out/linux-${page%%:*}.png"
    done
    test -f "$FIGGHOST_HOME/history.sqlite" && echo "history: ok" >>/out/run.log
    printf "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n" | /target/debug/figghost --mcp | head -c 120 >>/out/run.log; echo >>/out/run.log
  '
echo "pictures and log in $OUT"
