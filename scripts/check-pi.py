"""End-to-end check of the pi extension against the real program, on a private instance.

Writes the extension the way the settings page installs it (the socket of the private instance baked in), loads it
in node with a pretend pi (its `on` and its event bus), and plays a pi session: a turn, a question of
ask_user_question answered on the notch, one answered in the terminal, and pi without the app. It never touches
the real `~/.pi`, the real settings or the screen; the notch is outside every screen.

    python3 -I scripts/check-pi.py target/debug/app-ui
"""

import json
import os
import queue
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-pi.py <path to the program>")
HERE = os.path.dirname(os.path.abspath(__file__))
SOURCE = os.path.join(HERE, "..", "crates", "core-agents", "src", "pi_extension.ts")

root = tempfile.mkdtemp(prefix="figghost-pi.")
data = os.path.join(root, "data")
user = os.path.join(root, "user")
os.makedirs(data)
os.makedirs(os.path.join(user, ".pi", "agent", "extensions"))
with open(os.path.join(data, "config.json"), "w") as f:
    json.dump(
        {
            "agents": {"enabled": True, "approval_wait_seconds": 5, "sound": False, "peek_when_done": True, "open_on_ask": True},
            "show_settings_on_launch": False,
            "hotkey": "f13",
            "auto_paste": False,
            "tiling": {"enabled": False},
            "mouse": {"smooth": False},
        },
        f,
    )
env = dict(
    os.environ,
    FIGGHOST_HOME=data,
    FIGGHOST_AGENTS_HOME=user,
    FIGGHOST_NOTCH_OFFSCREEN="1",
    FIGGHOST_PASTEBOARD="figghost-pi-check",
    FIGGHOST_TRACE="1",
)
socket_path = os.path.join(data, "agents.sock")
trace_path = os.path.join(root, "trace.log")
extension = os.path.join(user, ".pi", "agent", "extensions", "figghost.ts")

# The pretend pi. Commands come on standard input, one per line; what the extension emits on the bus goes to
# standard output as JSON lines.
DRIVER = r"""
import readline from "node:readline";
import { pathToFileURL } from "node:url";
const [extension] = process.argv.slice(1);
const handlers = {}, bus = {};
const pi = {
  on: (name, fn) => { handlers[name] = fn; },
  events: {
    on: (name, fn) => { (bus[name] ??= []).push(fn); },
    emit: (name, data) => {
      if (name.startsWith("rpiv:ask-user:answer")) console.log(JSON.stringify({ emitted: name, data }));
      for (const fn of bus[name] ?? []) fn(data);
    },
  },
};
const ctx = { mode: "tui", cwd: "/Users/me/shop", sessionManager: { getSessionId: () => "pi-session-1" } };
(await import(pathToFileURL(extension).href)).default(pi);
const question = {
  question: "Which database should the shop use?", header: "Database", multiSelect: false,
  options: [{ label: "Postgres", description: "", hasPreview: false }, { label: "SQLite", description: "", hasPreview: false }],
};
for await (const line of readline.createInterface({ input: process.stdin })) {
  const [what, arg] = line.split(" ");
  const started = Date.now();
  if (what === "on") await handlers[arg]?.({ type: arg, aborted: false }, ctx);
  if (what === "ask") pi.events.emit("rpiv:ask-user:prompt", { toolCallId: arg, questions: [question] });
  if (what === "closed") pi.events.emit("rpiv:ask-user:blocked", { active: false, toolCallId: arg });
  console.log(JSON.stringify({ done: line, ms: Date.now() - started }));
}
process.exit(0);
"""

PRESS_BUTTON = """
function run(argv) {
  const proc = Application('System Events').processes.whose({ unixId: parseInt(argv[0]) })[0];
  for (const window of proc.windows()) {
    for (const element of window.entireContents()) {
      let role = '', name = '';
      try { role = element.role(); name = element.description(); } catch (e) { continue; }
      if (role === 'AXButton' && name === argv[1]) {
        element.actions.byName('AXPress').perform();
        return 'pressed';
      }
    }
  }
  return 'not found';
}
"""

LIST_BUTTONS = """
function run(argv) {
  const proc = Application('System Events').processes.whose({ unixId: parseInt(argv[0]) })[0];
  const names = [];
  for (const window of proc.windows()) {
    for (const element of window.entireContents()) {
      try { if (element.role() === 'AXButton') names.push(element.description()); } catch (e) {}
    }
  }
  return names.join(', ');
}
"""

failures = []
process = None


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def start():
    global process
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=open(trace_path, "a"), stdout=subprocess.DEVNULL)
    deadline = time.time() + 15
    while time.time() < deadline:
        probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            probe.connect(socket_path)
            return True
        except OSError:
            time.sleep(0.05)
        finally:
            probe.close()
    return False


def stop():
    global process
    if process and process.poll() is None:
        process.kill()
        process.wait()
    process = None


def trace():
    with open(trace_path) as f:
        return f.read()


def wait_for(text, count=1, seconds=10):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if trace().count(text) >= count:
            return True
        time.sleep(0.1)
    return False


class Pi:
    def __init__(self):
        self.node = subprocess.Popen(
            ["node", "--input-type=module", "-e", DRIVER, extension],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1,
        )
        self.lines = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()
        self.emitted = []

    def _read(self):
        for line in self.node.stdout:
            self.lines.put(json.loads(line))

    def _take(self, until):
        try:
            out = self.lines.get(timeout=max(0.0, until - time.time()))
        except queue.Empty:
            return None
        if "emitted" in out:
            self.emitted.append(out)
        return out

    def send(self, line, seconds=10):
        """Sends a command and waits until the pretend pi has run it; returns how long pi was held, in ms."""
        self.node.stdin.write(line + "\n")
        self.node.stdin.flush()
        until = time.time() + seconds
        while time.time() < until:
            out = self._take(until)
            if out and out.get("done") == line:
                return out["ms"]
        return None

    def next_emitted(self, seconds):
        """Waits for what the extension emits on its own, such as the answer from the notch."""
        until = time.time() + seconds
        while not self.emitted and time.time() < until:
            self._take(until)
        return self.emitted.pop(0) if self.emitted else None

    def close(self):
        self.node.stdin.close()
        try:
            self.node.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.node.kill()
        return self.node.stderr.read()


def press_when_shown(button, seconds=15):
    deadline = time.time() + seconds
    outcome = "not found"
    while time.time() < deadline:
        done = subprocess.run(
            ["osascript", "-l", "JavaScript", "-e", PRESS_BUTTON, str(process.pid), button], capture_output=True, timeout=60
        )
        outcome = done.stdout.decode().strip() if done.returncode == 0 else done.stderr.decode().strip()
        if outcome != "not found":
            return outcome
        time.sleep(0.2)
    return outcome


try:
    # The extension as the settings page writes it: the source with this instance's socket in place of the mark.
    with open(SOURCE) as f:
        text = f.read().replace('"__FIGGHOST_SOCKET__"', json.dumps(socket_path), 1)
    with open(extension, "w") as f:
        f.write(text)

    # 1. Without the app, pi is never held up and nothing goes wrong.
    pi = Pi()
    held = [pi.send(f"on {name}") for name in ("session_start", "agent_start", "agent_settled")]
    held.append(pi.send("ask call-0"))
    held.append(pi.send("on session_shutdown"))
    errors = pi.close()
    check(all(ms is not None and ms < 100 for ms in held), f"without the app every event returns at once ({held} ms)")
    check(errors.strip() == "", "and nothing is printed: " + errors.strip()[:200])

    check(start(), "the program starts and opens its agent socket")

    # 2. A turn of a pi session reaches the program as pi's.
    pi = Pi()
    for name in ("session_start", "agent_start", "agent_settled"):
        pi.send(f"on {name}")
    for kind in ("SessionStart", "PromptSubmit", "Stop"):
        check(wait_for(f"agents: {kind} from pi (shop)"), f"the program saw {kind} from pi")

    # 3. A question is put on the notch and stays past the wait of the settings while pi asks in its terminal too.
    pi.send("ask call-1")
    check(wait_for("agents: Question from pi (shop), waiting for an answer"), "the question reached the program and waits")
    outcome = press_when_shown("SQLite")
    if outcome.startswith("pressed"):
        answer = pi.next_emitted(5)
        check(
            answer is not None
            and answer["emitted"] == "rpiv:ask-user:answer"
            and answer["data"] == {"toolCallId": "call-1", "answers": [{"questionIndex": 0, "optionIndexes": [1]}]},
            "a choice on the notch becomes pi's answer by index: " + json.dumps(answer),
        )
        check(wait_for("answered (delivered)"), "the program logged that the answer was delivered")
        pi.send("closed call-1")
    else:
        names = subprocess.run(["osascript", "-l", "JavaScript", "-e", LIST_BUTTONS, str(process.pid)], capture_output=True).stdout.decode()
        print(f"skip  answering on the notch: {outcome[:90]} (buttons: {names.strip()[:300]})")
        pi.send("closed call-1")

    # 4. A question answered in the terminal: pi closes it, and the notch lets it go without answering.
    closed = trace().count("closed without an answer")
    pi.send("ask call-2")
    check(wait_for("agents: Question from pi (shop), waiting for an answer", 2), "a second question waits")
    time.sleep(7)
    check(trace().count("closed without an answer") == closed, "it is still there after the 5 seconds of the setting")
    pi.send("closed call-2")
    check(wait_for("closed without an answer", closed + 1), "closed in the terminal, it leaves the notch")
    check(pi.next_emitted(1) is None, "and nothing was answered for it")

    # 5. Claude Code run by pi's claude-bridge provider runs the user's Claude hooks too; they stay quiet, since pi
    # reports that session itself.
    bridge = dict(env, PI_CODING_AGENT="true", CLAUDE_CODE_ENTRYPOINT="sdk-ts")
    payload = json.dumps({"hook_event_name": "Stop", "session_id": "bridge-1", "cwd": "/Users/me/bridged"}).encode()
    done = subprocess.run([BIN, "--figghost-hook", "claude"], input=payload, capture_output=True, env=bridge, timeout=30)
    plain = dict(env, PI_CODING_AGENT="true")
    subprocess.run([BIN, "--figghost-hook", "claude"], input=payload.replace(b"bridged", b"by-hand"), capture_output=True, env=plain, timeout=30)
    check(wait_for("Stop from claude (by-hand)"), "a Claude Code started by hand in pi's terminal is still followed")
    check(done.returncode == 0 and done.stdout == b"" and "(bridged)" not in trace(), "a hook inside pi's bridge tells the program nothing")

    # 6. The end of the session.
    pi.send("on session_shutdown")
    check(wait_for("agents: SessionEnd from pi (shop)"), "the end of the session reaches the program")
    errors = pi.close()
    check(errors.strip() == "", "the extension printed nothing: " + errors.strip()[:200])
    check(process.poll() is None, "the program is still running")
finally:
    stop()
    shutil.rmtree(root, ignore_errors=True)

print("pi e2e ok" if not failures else f"pi e2e broken: {len(failures)} failure(s)")
sys.exit(1 if failures else 0)
