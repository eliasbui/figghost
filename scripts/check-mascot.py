#!/usr/bin/env python3
"""End to end check of the clipboard mascot through the real program, on a private instance: its own data
folder, a named pasteboard instead of the clipboard, and the notch (with the stage under it) far outside
every screen, so nothing shows and nothing of the user's is touched.

    scripts/check-mascot.py <program> <named_board>

<program> is target/debug/app-ui (or the app's binary), <named_board> the macos-sys example of that name
(cargo build -p macos-sys --example named_board). Checks that a copy makes the chosen character play, that
nothing plays while the mascot is off, and that the program is still running after each animation.
"""
import json
import os
import subprocess
import sys
import tempfile
import time

BIN, BOARD = sys.argv[1], sys.argv[2]
PASTEBOARD = "figghost-mascot-check"
root = tempfile.mkdtemp(prefix="figghost-mascot-")
data = os.path.join(root, "data")
os.makedirs(data)
log = os.path.join(root, "err.log")
failures = []
process = None


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def write_config(enabled, character):
    with open(os.path.join(data, "config.json"), "w") as f:
        json.dump(
            {
                "mascot": {"enabled": enabled, "character": character, "farewell": "regret"},
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
    FIGGHOST_NOTCH_OFFSCREEN="1",
    FIGGHOST_PASTEBOARD=PASTEBOARD,
    FIGGHOST_AGENTS_HOME=os.path.join(root, "user"),
    FIGGHOST_TRACE="1",
)


def start():
    global process
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=open(log, "a"), stdout=subprocess.DEVNULL)
    time.sleep(2.0)
    check(process.poll() is None, "the program is running")


def stop():
    global process
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(5)
        except subprocess.TimeoutExpired:
            process.kill()
    process = None


def copy(text):
    subprocess.run([BOARD, PASTEBOARD, "write", text], check=True, capture_output=True)


def fetches():
    with open(log) as f:
        return [line for line in f if "mascot:" in line and "fetches" in line]


try:
    write_config(True, "wizard")
    start()
    copy("the mascot fetches this")
    time.sleep(2.5)
    lines = fetches()
    check(len(lines) == 1, f"one copy, one fetch: {lines}")
    check(any("wizard fetches Text" in line for line in lines), "the chosen character fetches a text chip")
    check(process.poll() is None, "still running after the animation")
    copy("and this one too")
    time.sleep(2.5)
    check(len(fetches()) == 2, "a second copy plays again")
    stop()

    write_config(False, "spider")
    start()
    before = len(fetches())
    copy("nothing should come out for this")
    time.sleep(1.5)
    check(len(fetches()) == before, "nothing plays while the mascot is off")
    check(process.poll() is None, "still running")
finally:
    stop()
    subprocess.run([BOARD, PASTEBOARD, "release"], capture_output=True)

print("PASS" if not failures else f"FAILED: {len(failures)}")
sys.exit(1 if failures else 0)
