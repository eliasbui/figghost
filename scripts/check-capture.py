"""Startup check of the capture feature through the real program, on a private instance.

Starts the program with its own data folder, a private pasteboard and the notch outside every screen, with
the capture shortcuts moved to function keys that hardly any keyboard has, so that nothing here can start a
capture or show anything. It checks what can be checked without the Screen Recording permission and without a
screen: that the feature starts when it is switched on, registers its global shortcuts, reports a shortcut that is
taken, survives a hand-edited config with hostile values, uses no CPU while idle,
and leaves no process behind. The capture itself is not run here (see HANDOFF.md).

    python3 -I scripts/check-capture.py target/debug/app-ui
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-capture.py <path to the program>")
BASE = {
    "show_settings_on_launch": False,
    "hotkey": "f13",
    "auto_paste": False,
    "tiling": {"enabled": False},
    "mouse": {"smooth": False},
}
FUNCTION_KEYS = {"region": "f17", "window": "f18", "screen": "f19", "repeat": "f20", "text": ""}
root = tempfile.mkdtemp(prefix="figghost-capture.")
failures = []
children = []


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def launch(name, capture, raw=None):
    """Starts a private instance; returns (process, data folder, stderr path)."""
    data = os.path.join(root, name)
    os.makedirs(data)
    with open(os.path.join(data, "config.json"), "w") as f:
        f.write(raw if raw is not None else json.dumps(dict(BASE, capture=capture)))
    env = dict(
        os.environ,
        FIGGHOST_HOME=data,
        FIGGHOST_NOTCH_OFFSCREEN="1",
        FIGGHOST_PASTEBOARD="figghost-capture-check",
        FIGGHOST_AGENTS_HOME=os.path.join(root, "agents-home"),
    )
    log = os.path.join(root, name + ".log")
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=open(log, "w"), stdout=subprocess.DEVNULL)
    children.append(process)
    return process, data, log


def stop(process):
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def cpu(process):
    out = subprocess.run(["ps", "-o", "%cpu=", "-p", str(process.pid)], capture_output=True, text=True).stdout
    return float(out.strip() or 0)


def text(path):
    with open(path) as f:
        return f.read()


try:
    # 1. On, with shortcuts on function keys: it starts, says nothing wrong and idles.
    first, _, first_log = launch("first", {"enabled": True, "shortcuts": FUNCTION_KEYS})
    time.sleep(3)
    check(first.poll() is None, "the program runs with capture switched on")
    check("capture:" not in text(first_log) and "panicked" not in text(first_log), "no capture error at start: " + text(first_log).strip()[:200])
    samples = []
    for _ in range(5):
        samples.append(cpu(first))
        time.sleep(0.6)
    check(max(samples) < 1.0, f"idle CPU stays under 1 percent ({samples})")

    # 2. Two modes on one shortcut (a hand-edited file): the system refuses the second registration, and the
    # program reports it instead of crashing. (Two programs asking for the same keys do not refuse each other:
    # the system lets the newest win, so that is not checked.)
    clash = dict(FUNCTION_KEYS, window="f17")
    second, _, second_log = launch("second", {"enabled": True, "shortcuts": clash})
    time.sleep(3)
    check(second.poll() is None, "the program still runs when two modes share a shortcut")
    check("capture: taken" in text(second_log), "a shortcut that is taken is reported: " + text(second_log).strip()[:200])
    stop(second)

    # 3. Off by default: with no capture section it registers nothing, so the same keys are free again.
    third, _, third_log = launch("third", {}, raw=json.dumps(dict(BASE)))
    time.sleep(2)
    check(third.poll() is None and "capture:" not in text(third_log), "off by default and quiet")
    stop(third)

    # 4. Hostile values in the file: wrong shortcuts and a folder that points at a system folder. The
    # program starts, and the bad shortcut is skipped with a line in the log.
    hostile = {
        "enabled": True,
        "folder": "../../../../etc",
        "shortcuts": {"region": "not a shortcut", "window": "cmd+cmd+cmd", "screen": "f16", "bogus": "f1"},
    }
    fourth, _, fourth_log = launch("fourth", hostile)
    time.sleep(3)
    check(fourth.poll() is None, "a hostile config does not stop the program")
    check("panicked" not in text(fourth_log), "no panic on hostile values")
    check("shortcut for region" in text(fourth_log), "a shortcut that does not parse is skipped and logged")
    stop(fourth)

    # 5. A section of the wrong type: the file is moved aside, never overwritten, and defaults are used.
    broken = json.dumps(dict(BASE, capture={"enabled": "yes", "copy": 3}))
    fifth, data, _ = launch("fifth", None, raw=broken)
    time.sleep(2)
    check(fifth.poll() is None, "a config of the wrong type does not stop the program")
    check(os.path.exists(os.path.join(data, "config.json.bad")), "the unreadable file is moved aside")
    stop(fifth)

    stop(first)
    check(first.poll() is not None, "the first program exits when told to")
finally:
    for process in children:
        if process.poll() is None:
            process.kill()
    shutil.rmtree(root, ignore_errors=True)

print("FAILED" if failures else "capture startup checks passed")
sys.exit(1 if failures else 0)
