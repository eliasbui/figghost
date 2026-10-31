#!/bin/sh
# Starts the real program on a private instance with Jarvis off, then on and pointed at a made-up service on a
# loopback port (FIGGHOST_JARVIS_URL), and checks through its trace that an off feature does nothing, that an on one
# opens its panel, sends a question typed for it (FIGGHOST_JARVIS_ASK, since nobody can type here) and takes the
# answer in, and that with a service that is not there it says so without dying. Neither the microphone nor the
# voice is used (voice and speak are off), and nothing of the real settings or the real Keychain is touched: the
# service is a local model, which has no key.
#
# Usage: scripts/check-jarvis.sh [program]    (default: target/debug/app-ui)
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROGRAM=${1:-${CARGO_TARGET_DIR:-$ROOT/target}/debug/app-ui}
WORK=$(mktemp -d)
PID=""
SERVER=""
FAILED=0

cleanup() {
    for p in "$PID" "$SERVER"; do
        if [ -n "$p" ]; then kill "$p" 2>/dev/null || true; wait "$p" 2>/dev/null || true; fi
    done
    rm -rf "$WORK"
}
trap cleanup EXIT

pass() { echo "ok     $1"; }
fail() { echo "FAILED $1"; FAILED=1; }

# A service that answers any request with three pieces of text, and keeps the request it was sent.
cat > "$WORK/server.py" <<'PY'
import http.server, sys

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"]))
        open(sys.argv[2], "wb").write(body)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        open(sys.argv[2] + ".headers", "w").write(str(self.headers))
        codex = b'"input"' in body
        for piece in ["Xin ", "chao ", "ban."]:
            if codex:
                line = 'data: {"type":"response.output_text.delta","delta":"%s"}\n\n' % piece
            else:
                line = 'data: {"choices":[{"delta":{"content":"%s"}}]}\n\n' % piece
            self.wfile.write(line.encode())
            self.wfile.flush()
        if codex:
            self.wfile.write(b'data: {"type":"response.completed","response":{}}\n\n')
        else:
            self.wfile.write(b"data: [DONE]\n\n")
    def log_message(self, *args):
        pass

http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
PY

# Starts the program with the Jarvis settings in $1 and leaves its trace in $WORK/$2.log; $3 is the service address.
run() {
    mkdir -p "$WORK/home"
    cat > "$WORK/home/config.json" <<JSON
{ "show_settings_on_launch": false, "jarvis": $1 }
JSON
    FIGGHOST_HOME="$WORK/home" FIGGHOST_NOTCH_OFFSCREEN=1 FIGGHOST_TRACE=1 FIGGHOST_JARVIS_START=1 \
        FIGGHOST_JARVIS_ASK="What is the date? Say it \"plainly\"." FIGGHOST_JARVIS_URL="$3" \
        "$PROGRAM" --background 2> "$WORK/$2.log" &
    PID=$!
}

stop() {
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
    PID=""
}

PORT=$((20000 + $$ % 20000))
URL="http://127.0.0.1:$PORT/v1/chat/completions"
ON='{ "enabled": true, "provider": "local", "model": "test-model", "voice": false, "speak": false, "sound": false, "future_key": 1 }'

run '{ "enabled": false }' off "$URL"
sleep 5
if kill -0 "$PID" 2>/dev/null; then pass "the program runs with Jarvis off"; else fail "the program exited"; cat "$WORK/off.log"; exit 1; fi
if grep -q "jarvis:" "$WORK/off.log"; then fail "an off feature did something"; grep "jarvis:" "$WORK/off.log"; else pass "an off feature does nothing"; fi
stop

python3 "$WORK/server.py" "$PORT" "$WORK/request.json" &
SERVER=$!
sleep 1
run "$ON" on "$URL"
sleep 7
if grep -q "jarvis: cannot register" "$WORK/on.log"; then fail "the shortcut was not registered"; else pass "the shortcut is registered"; fi
if grep -q "jarvis: the panel opens" "$WORK/on.log"; then pass "the panel opens"; else fail "the panel did not open"; fi
if grep -q "jarvis: asking local (test-model)" "$WORK/on.log"; then pass "the question is sent to the model that was chosen"; else fail "no question was sent"; fi
if grep -q "jarvis: answered with 13 characters" "$WORK/on.log"; then pass "the three pieces of the answer arrive whole"; else fail "the answer did not arrive"; grep "jarvis" "$WORK/on.log" || true; fi
if [ -f "$WORK/request.json" ] && grep -q '"stream": *true' "$WORK/request.json" && grep -q 'test-model' "$WORK/request.json" \
    && grep -q 'plainly' "$WORK/request.json"; then pass "the request carried the model, the stream and the question"; else fail "the request was wrong"; cat "$WORK/request.json" 2>/dev/null || true; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running after the answer"; else fail "the program died"; tail -20 "$WORK/on.log"; fi
stop

# Codex: Jarvis signs in to ChatGPT by itself and keeps the session in the Keychain; the sign-in of the Codex
# command line is never read. A made-up one in a folder of its own (CODEX_HOME) must therefore send nothing. Skipped
# when this Mac already has a session of Jarvis, which would rightly be used.
if security find-generic-password -s vn.figghost.jarvis -a codex-session >/dev/null 2>&1; then
    echo "skip: this Mac has a ChatGPT session of Jarvis, so the check that Codex's own is ignored is not run"
else
    mkdir -p "$WORK/codex"
    cat > "$WORK/codex/auth.json" <<JSON
{ "tokens": { "access_token": "e30.eyJleHAiOjQxMDI0NDQ4MDB9.sig", "account_id": "acct-check", "refresh_token": "never-used" } }
JSON
    rm -f "$WORK/request.json" "$WORK/request.json.headers"
    CODEX_HOME="$WORK/codex" run '{ "enabled": true, "provider": "codex", "voice": false, "speak": false, "sound": false }' nosign "$URL"
    sleep 6
    if grep -q "jarvis: asking" "$WORK/nosign.log" || [ -f "$WORK/request.json" ]; then fail "the sign-in of the Codex program was used"; else pass "the sign-in of the Codex program is never used"; fi
    if grep -q "never-used" "$WORK/codex/auth.json"; then pass "the Codex sign-in file is left as it was"; else fail "the Codex sign-in file was changed"; fi
    if kill -0 "$PID" 2>/dev/null; then pass "the program is still running with no sign-in"; else fail "the program died with no sign-in"; fi
    stop
fi
kill "$SERVER" 2>/dev/null || true; wait "$SERVER" 2>/dev/null || true; SERVER=""

# With nothing at the address the question fails and says so.
run "$ON" gone "http://127.0.0.1:$((PORT + 1))/v1/chat/completions"
sleep 7
if grep -q "jarvis: failed with Unreachable" "$WORK/gone.log"; then pass "a service that is not there is reported as unreachable"; else fail "no failure was reported"; grep "jarvis" "$WORK/gone.log" || true; fi
if kill -0 "$PID" 2>/dev/null; then pass "the program is still running after the failure"; else fail "the program died after the failure"; fi
if grep -q "panicked" "$WORK"/*.log; then fail "the program panicked"; grep -h "panicked" -A 3 "$WORK"/*.log; fi
stop
exit "$FAILED"
