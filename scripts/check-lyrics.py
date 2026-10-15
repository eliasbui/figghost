"""Smoke check of the lyrics feature through the real program, on a private instance.

Starts the program with its own data folder, a pretend home for the agents' files and a notch placed outside
every screen, with lyrics shown and online fetching off. The notch only exists on screen while something opens
it, so a pretend permission request from an agent opens it; then the Media tab is opened through the
accessibility tree (no pointer, no keys), left open for a while, and another tab is opened. It checks that the program survives, that nothing is fetched or
written to the lyrics cache while fetching is off, and that it is idle again once the tab is left.

A playing track is not simulated: Music and Spotify are never started by this check, and if one of them
is already running the check stops before touching anything.

    python3 -I scripts/check-lyrics.py target/debug/app-ui
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-lyrics.py <path to the program>")

for player in ("Music", "Spotify"):
    if subprocess.run(["pgrep", "-x", player], capture_output=True).returncode == 0:
        sys.exit(f"{player} is running; not starting a program that would ask it questions")

root = tempfile.mkdtemp(prefix="figghost-lyrics.")
data = os.path.join(root, "data")
user = os.path.join(root, "user")
os.makedirs(data)
os.makedirs(os.path.join(user, ".claude"))
os.makedirs(os.path.join(user, ".codex"))
with open(os.path.join(data, "config.json"), "w") as f:
    json.dump(
        {
            "lyrics": {"enabled": True, "fetch": False},
            "agents": {"enabled": True, "approval_wait_seconds": 60, "sound": False, "peek_when_done": False},
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
    FIGGHOST_PASTEBOARD="figghost-lyrics-check",
)
failures = []
process = None


def skip(message):
    print("skip  " + message)


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


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
    done = subprocess.run(
        ["osascript", "-l", "JavaScript", "-e", PRESS_BUTTON, str(process.pid), button],
        capture_output=True,
        timeout=60,
    )
    return done.stdout.decode().strip() if done.returncode == 0 else done.stderr.decode().strip()


def press_when_shown(button, seconds=15):
    deadline = time.time() + seconds
    outcome = "not found"
    while time.time() < deadline:
        outcome = press(button)
        if outcome == "pressed":
            return outcome
        time.sleep(0.3)
    return outcome


def cpu_percent():
    out = subprocess.run(["ps", "-o", "%cpu=", "-p", str(process.pid)], capture_output=True).stdout.decode()
    return float(out.strip() or 0)


def children():
    out = subprocess.run(["pgrep", "-P", str(process.pid)], capture_output=True).stdout.decode()
    return [p for p in out.split() if p]


waiting = None
try:
    process = subprocess.Popen([BIN, "--background"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    time.sleep(3)
    check(process.poll() is None, "the program starts with lyrics switched on")

    waiting = subprocess.Popen([BIN, "--figghost-hook", "claude"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)
    request = {
        "hook_event_name": "PermissionRequest",
        "session_id": "s1",
        "cwd": "/Users/me/shop",
        "tool_name": "Bash",
        "tool_input": {"command": "ls"},
    }
    waiting.stdin.write(json.dumps(request).encode())
    waiting.stdin.close()

    outcome = press_when_shown("Media")
    if outcome != "pressed":
        # Same limit as scripts/check-agents.py: the buttons of a window outside the screen are not always
        # reachable by this process, which is then a missing permission and not a failure of the program.
        skip(f"opening the Media tab through accessibility: {outcome}")
    time.sleep(4)
    check(process.poll() is None, "the program stays up, with the Media tab open if it could be opened, and no player running")
    check(not os.path.exists(os.path.join(data, "lyrics-cache")), "nothing is written to the lyrics cache while fetching is off")

    if outcome == "pressed":
        check(press_when_shown("Clipboard") == "pressed", "another tab can be opened again")
    time.sleep(3)
    busy = cpu_percent()
    check(process.poll() is None and busy < 1.0, f"idle ({busy:.1f}% CPU, helpers: {len(children())})")
finally:
    if waiting and waiting.poll() is None:
        waiting.kill()
    if process and process.poll() is None:
        process.kill()
        process.wait()
    err = process.stderr.read().decode() if process and process.stderr else ""
    if err.strip():
        print("stderr of the program:\n" + err)
    shutil.rmtree(root, ignore_errors=True)

if failures:
    sys.exit(f"{len(failures)} check(s) failed")
print("lyrics check passed")
