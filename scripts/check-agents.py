"""End-to-end check of the agents feature through the real program, on a private instance.

Starts the program with its own data folder, a pretend home for the agents' files and a notch placed
outside every screen, so it never touches the real settings, the real `~/.claude` or `~/.codex`, or the
screen. It then plays the part of an agent: runs the hook command the way Claude Code and Codex do, and
checks what the program does with each event and that it can never get in an agent's way.

    python3 -I scripts/check-agents.py target/debug/app-ui
"""

import json
import os
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-agents.py <path to the program>")
# Long enough for the slow accessibility calls that press the buttons, short enough to keep the run quick.
WAIT_SECONDS = 12
WAITING = "waiting for an answer"

root = tempfile.mkdtemp(prefix="figghost-agents.")
data = os.path.join(root, "data")
user = os.path.join(root, "user")
os.makedirs(data)
os.makedirs(os.path.join(user, ".claude"))
os.makedirs(os.path.join(user, ".codex"))
for name in (".claude/settings.json", ".codex/hooks.json"):
    with open(os.path.join(user, name), "w") as f:
        f.write("{}\n")
with open(os.path.join(data, "config.json"), "w") as f:
    json.dump(
        {
            "agents": {"enabled": True, "approval_wait_seconds": WAIT_SECONDS, "sound": False, "peek_when_done": False},
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
    FIGGHOST_PASTEBOARD="figghost-agents-check",
    FIGGHOST_TRACE="1",
)
socket_path = os.path.join(data, "agents.sock")
trace_path = os.path.join(root, "trace.log")

def screen_is_locked():
    """While the screen is locked the accessibility tree of other apps is empty: a check that needs it cannot
    say anything, which is different from failing."""
    done = subprocess.run(
        ["osascript", "-l", "JavaScript", "-e",
         "ObjC.import('AppKit'); ObjC.unwrap($.NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier)"],
        capture_output=True,
    )
    return done.stdout.decode().strip() == "com.apple.loginwindow"


failures = []
process = None


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def start():
    global process
    trace = open(trace_path, "a")
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=trace, stdout=subprocess.DEVNULL)
    # A killed program leaves its socket file behind, so the file existing proves nothing: wait until
    # somebody accepts a connection.
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


def hook(agent, payload, raw=None):
    started = time.monotonic()
    done = subprocess.run(
        [BIN, "--figghost-hook", agent],
        input=raw if raw is not None else json.dumps(payload).encode(),
        capture_output=True,
        env=env,
        timeout=60,
    )
    return time.monotonic() - started, done.stdout.decode(), done.returncode


def event(name, session="s1", **more):
    return dict({"hook_event_name": name, "session_id": session, "cwd": "/Users/me/shop"}, **more)


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


def press(button):
    """Presses a button of the program's own window through the accessibility tree: no pointer, no keys, and
    it works while the window is far outside every screen. Returns "pressed", "not found" or an error text."""
    done = subprocess.run(
        ["osascript", "-l", "JavaScript", "-e", PRESS_BUTTON, str(process.pid), button],
        capture_output=True,
        timeout=60,
    )
    return done.stdout.decode().strip() if done.returncode == 0 else done.stderr.decode().strip()


def ask(session, command):
    """Starts a hook the way an agent does and leaves it waiting for an answer."""
    waiting = subprocess.Popen(
        [BIN, "--figghost-hook", "claude"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env
    )
    payload = event("PermissionRequest", session=session, tool_name="Bash", tool_input={"command": command})
    waiting.stdin.write(json.dumps(payload).encode())
    waiting.stdin.close()
    waiting.started = time.monotonic()
    return waiting


def answer_of(waiting):
    out = waiting.stdout.read().decode()
    waiting.wait(timeout=30)
    return out, time.monotonic() - waiting.started


def wait_for_count(text, count, seconds=10):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if trace().count(text) >= count:
            return True
        time.sleep(0.1)
    return False


def behavior(out):
    try:
        return json.loads(out)["hookSpecificOutput"]["decision"]["behavior"]
    except (ValueError, KeyError):
        return None


def press_when_shown(button, seconds=15):
    deadline = time.time() + seconds
    outcome = "not found"
    while time.time() < deadline:
        outcome = press(button)
        if outcome != "not found":
            return outcome
        time.sleep(0.2)
    return outcome


def trace():
    with open(trace_path) as f:
        return f.read()


try:
    # 1. No app running: the agent must carry on as if the hook did not exist.
    took, out, code = hook("claude", event("PermissionRequest", tool_name="Bash", tool_input={"command": "ls"}))
    check(code == 0 and out == "" and took < 1.5, f"without the app a request ends quietly at once ({took:.2f} s)")

    # 2. Start the app with the feature on.
    check(start(), "the program starts and opens its agent socket")
    mode = stat.S_IMODE(os.stat(socket_path).st_mode)
    check(mode == 0o600, f"the socket is private to the user (mode {mode:o})")

    # 3. Plain events are acknowledged at once and reach the tracker.
    for name in ("SessionStart", "UserPromptSubmit", "Stop"):
        took, out, code = hook("claude", event(name))
        check(code == 0 and out == "" and took < 0.5, f"{name} returns quietly in {took * 1000:.0f} ms")
    time.sleep(0.5)
    log = trace()
    for kind in ("SessionStart", "PromptSubmit", "Stop"):
        check(f"agents: {kind} from claude (shop)" in log, f"the program saw {kind}")

    # 4. Codex speaks the same protocol, with its own command shape.
    took, out, code = hook("codex", event("Stop", session="c1"))
    check(code == 0 and took < 0.5, "a Codex event works the same")

    # 5. A permission request nobody answers falls back to the agent's own prompt after the wait.
    started = time.monotonic()
    took, out, code = hook("claude", event("PermissionRequest", session="s2", tool_name="Bash", tool_input={"command": "rm -rf build"}))
    check(code == 0 and out == "", "an unanswered request prints nothing, so the agent asks in its terminal")
    check(WAIT_SECONDS - 0.7 <= took <= WAIT_SECONDS + 2.5, f"it waited about {WAIT_SECONDS} s ({took:.1f} s)")
    time.sleep(1.6)
    log = trace()
    check("PermissionRequest from claude (shop), waiting for an answer" in log, "the program showed the question")
    check("closed without an answer" in log, "the program dropped the question once the wait was over")


    # 5b. The buttons on the notch answer a request. The notch is far outside every screen, so this works
    # while the screen is locked and never touches the real pointer or keyboard.
    seen = trace().count(WAITING)
    first = ask("q1", "git push --force")
    check(wait_for_count(WAITING, seen + 1), "a new request reaches the program and waits")
    outcome = press_when_shown("Allow")
    if outcome.startswith("pressed"):
        out, took = answer_of(first)
        check(behavior(out) == "allow", "Allow on the notch makes the hook print an allow decision")
        check(took < WAIT_SECONDS - 2, f"and it answered at once, not by timing out ({took:.1f} s)")
        check(json.loads(out)["hookSpecificOutput"]["hookEventName"] == "PermissionRequest", "the decision names its event")
        check(wait_for_count("allowed (delivered)", 1), "the program logged that the answer was delivered")

        # Two requests at once are answered one after the other, in the order they came.
        seen = trace().count(WAITING)
        one = ask("q2", "echo one")
        check(wait_for_count(WAITING, seen + 1), "the first of two requests is waiting")
        two = ask("q3", "echo two")
        check(wait_for_count(WAITING, seen + 2), "the second one is waiting behind it")
        time.sleep(0.5)
        check(press_when_shown("Allow").startswith("pressed"), "the first request is answered with Allow")
        out, took = answer_of(one)
        check(behavior(out) == "allow" and took < WAIT_SECONDS - 2, f"it was the first one that was allowed ({took:.1f} s)")
        check(press_when_shown("Deny").startswith("pressed"), "then the second request is shown and answered with Deny")
        out, took = answer_of(two)
        decision = json.loads(out)["hookSpecificOutput"]["decision"] if out else {}
        check(decision.get("behavior") == "deny" and decision.get("message"), "Deny makes the hook print a deny decision with a reason")
        check(took < WAIT_SECONDS - 1, f"and the second one was answered before its wait ended ({took:.1f} s)")
        check(wait_for_count("denied (delivered)", 1), "the program logged the refusal")
        # With nothing left to answer there are no buttons.
        check(press("Allow") == "not found", "no buttons are left once every request is answered")
    else:
        # No permission to read the window through accessibility: say so, and let the request time out.
        answer_of(first)
        why = "the screen is locked" if screen_is_locked() else outcome[:90]
        print(f"skip  answering through the buttons: {why}")

    # 6. A huge tool input is reduced before it reaches the program, and the request still works.
    big = event("PermissionRequest", session="s3", tool_name="Bash", tool_input={"command": "y" * 1_500_000})
    started = time.monotonic()
    took, out, code = hook("codex", big)
    check(code == 0 and out == "" and took < WAIT_SECONDS + 3, f"a 1.5 MB command is handled ({took:.1f} s)")

    # 7. Anything that is not a hook payload is ignored.
    for junk in (b"", b"not json", b"[1,2]", b"{}"):
        took, out, code = hook("claude", None, raw=junk)
        check(code == 0 and out == "" and took < 0.5, f"input {junk[:12]!r} is ignored")
    took, out, code = hook("unknown-agent", event("Stop"))
    check(code == 0 and out == "", "an unknown agent name is ignored")

    # 7b. The program disappears while a request is waiting: the agent must be released at once, not after
    # the wait, and must be left to ask in its own terminal.
    seen = trace().count(WAITING)
    stranded = ask("q9", "ls")
    check(wait_for_count(WAITING, seen + 1), "a request is waiting when the program goes away")
    stop()
    out, took = answer_of(stranded)
    check(out == "" and took < 3, f"the hook is released at once and prints nothing ({took:.1f} s)")
    check(stranded.returncode == 0, "and it exits cleanly")

    # 8. A program that was killed leaves a stale socket; the next start replaces it and listens again.
    took, out, code = hook("claude", event("PermissionRequest", tool_name="Bash", tool_input={"command": "ls"}))
    check(code == 0 and out == "" and took < 1.5, "after the app is gone a request ends quietly at once")
    check(os.path.exists(socket_path), "a killed program leaves its socket file behind")
    check(start(), "the next start replaces the stale socket and listens again")
    before = trace().count("Stop from claude")
    hook("claude", event("Stop", session="s9"))
    time.sleep(0.5)
    check(trace().count("Stop from claude") == before + 1, "the restarted program receives events")
finally:
    stop()
    shutil.rmtree(root, ignore_errors=True)

print("agents e2e ok" if not failures else f"agents e2e broken: {len(failures)} failure(s)")
sys.exit(1 if failures else 0)
