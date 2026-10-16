#!/bin/sh
# Rebuilds and restarts the local Figghost bundle whenever source files change.
#
# Create a persistent local Code Signing certificate once in Keychain Access,
# then run: SIGN_IDENTITY="Figghost Local" scripts/dev.sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
APP="$ROOT/dist/Figghost.app"
BIN="$APP/Contents/MacOS/Figghost"
PID_FILE="$ROOT/.figghost-dev.pid"
IDENTITY=${SIGN_IDENTITY:-Figghost Local}

stop_app() {
    if [ ! -f "$PID_FILE" ]; then
        return
    fi

    PID=$(cat "$PID_FILE")
    COMMAND=$(ps -p "$PID" -o command= 2>/dev/null || true)
    case "$COMMAND" in
        *"$BIN"*) kill "$PID" 2>/dev/null || true ;;
    esac
    rm -f "$PID_FILE"
}

restart_app() {
    stop_app
    SIGN_IDENTITY="$IDENTITY" "$ROOT/scripts/bundle.sh"
    "$BIN" &
    echo "$!" > "$PID_FILE"
}

case ${1:-watch} in
    --restart)
        restart_app
        ;;
    --stop)
        stop_app
        ;;
    watch)
        if ! command -v cargo-watch >/dev/null 2>&1; then
            echo "cargo-watch is required; install it with: cargo install cargo-watch" >&2
            exit 1
        fi
        if [ "$IDENTITY" != "-" ] && ! security find-identity -v -p codesigning | grep -F -- "$IDENTITY" >/dev/null; then
            echo "Code-signing identity not found: $IDENTITY" >&2
            echo "Create it in Keychain Access (or use SIGN_IDENTITY=\"-\" for ad-hoc signing)." >&2
            exit 1
        fi

        export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$HOME/Library/Caches/figghost-target"}
        trap '"$ROOT/scripts/dev.sh" --stop; exit' INT TERM EXIT
        cargo watch \
            --watch "$ROOT/crates" \
            --watch "$ROOT/Cargo.toml" \
            --watch "$ROOT/Cargo.lock" \
            --watch "$ROOT/scripts/bundle.sh" \
            --shell "'$ROOT/scripts/dev.sh' --restart"
        ;;
    *)
        echo "usage: $0 [watch|--restart|--stop]" >&2
        exit 2
        ;;
esac
