"""Smoke check of the calendar feature through the real program, on a private instance.

Starts the program with its own data folder and a notch placed outside every screen, so it never touches the
real settings or the screen, and checks what can be checked without a calendar permission: that a launch with
the feature on reads the permission state but never asks, that a hostile config is survived, that nothing
runs while the feature is off, and that an idle program stays under one percent of a core. It cannot check
EventKit itself: reading events needs the Calendars permission, which a program run from a build folder
neither has nor can ask for (it carries no usage text).

    python3 -I scripts/check-calendar.py target/debug/app-ui
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-calendar.py <path to the program>")
root = tempfile.mkdtemp(prefix="figghost-calendar.")
failures = []


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def run(name, calendar, seconds=5.0):
    """Runs the program for a while with the given calendar config; returns its trace, exit status and CPU."""
    data = os.path.join(root, name)
    os.makedirs(data)
    config = {
        "show_settings_on_launch": False,
        "hotkey": "f13",
        "auto_paste": False,
        "tiling": {"enabled": False},
        "mouse": {"smooth": False},
        "calendar": calendar,
    }
    with open(os.path.join(data, "config.json"), "w") as f:
        json.dump(config, f)
    env = dict(
        os.environ,
        FIGGHOST_HOME=data,
        FIGGHOST_AGENTS_HOME=os.path.join(root, "user"),
        FIGGHOST_NOTCH_OFFSCREEN="1",
        FIGGHOST_PASTEBOARD="figghost-calendar-check",
        FIGGHOST_TRACE="1",
    )
    trace_path = os.path.join(data, "trace.log")
    with open(trace_path, "w") as trace:
        process = subprocess.Popen([BIN, "--background"], env=env, stderr=trace, stdout=subprocess.DEVNULL)
    time.sleep(seconds)
    alive = process.poll() is None
    cpu = None
    if alive:
        out = subprocess.run(["ps", "-o", "%cpu=,rss=", "-p", str(process.pid)], capture_output=True).stdout.split()
        cpu = (float(out[0]), int(out[1]) // 1024) if len(out) == 2 else None
        process.kill()
    process.wait()
    with open(trace_path) as f:
        text = f.read()
    return text, alive, cpu


try:
    text, alive, cpu = run("on", {"enabled": True})
    check(alive, "the program runs with the calendar on")
    check("calendar: events " in text, "the feature reads the permission state at launch")
    check("not_determined" in text or "denied" in text or "granted" in text, "the state is one the program knows")
    check("asking for" not in text, "launching never asks for access")
    check("panicked" not in text, "no panic in the log")
    if cpu:
        check(cpu[0] < 1.0, f"idle with the calendar on uses under 1 percent of a core ({cpu[0]}%, {cpu[1]} MB)")

    text, alive, cpu = run("off", {"enabled": False})
    check(alive and "calendar:" not in text, "with the calendar off the feature does nothing")
    if cpu:
        check(cpu[0] < 1.0, f"idle with the calendar off uses under 1 percent of a core ({cpu[0]}%, {cpu[1]} MB)")

    wrong_types = {"enabled": True, "lead_minutes": "soon", "excluded": ["a", 7, None], "sound": "loud"}
    text, alive, cpu = run("wrong", wrong_types, seconds=3.0)
    check(alive, "a config with the wrong types does not stop the program")
    check(os.path.exists(os.path.join(root, "wrong", "config.json.bad")), "that config is set aside, not overwritten")

    extreme = {
        "enabled": True,
        "lead_minutes": 4000000000,
        "excluded": ["a", "a", "x\ny", "z" * 5000],
        "future_key": {"nested": [1, 2, 3]},
    }
    text, alive, cpu = run("extreme", extreme, seconds=3.0)
    check(alive and "calendar: events" in text, "extreme values are clamped and the feature starts")
finally:
    shutil.rmtree(root, ignore_errors=True)

print()
print("all checks passed" if not failures else f"{len(failures)} checks failed")
sys.exit(1 if failures else 0)
