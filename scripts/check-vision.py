"""End-to-end check of the colour picker's start-up through the real program, on private instances.

Starts the program with its own data folder, a named pasteboard and a notch placed outside every screen, so
it never touches the real settings or clipboard and shows nothing. It checks what can be seen from outside
without posting an input event: that the program starts with the feature on, off or without a shortcut, that
a hostile config file is survived and left alone, and that an idle picker costs no CPU. Whether the shortcut
fires needs a real key press, which this does not post. The loupe itself is checked in process by
`cargo run -p macos-sys --example picker_check`.

    python3 -I scripts/check-vision.py target/debug/app-ui
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-vision.py <path to the program>")
# An unusual combination, so that a check never takes a shortcut somebody uses.
SHORTCUT = "ctrl+opt+cmd+f19"

root = tempfile.mkdtemp(prefix="figghost-vision.")
failures = []
running = []


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def start(name, vision, clipboard_key):
    """Starts an instance; returns (process, config path, stderr path)."""
    data = os.path.join(root, name)
    os.makedirs(data)
    config = os.path.join(data, "config.json")
    with open(config, "w") as f:
        json.dump(
            {
                "vision": vision,
                "show_settings_on_launch": False,
                "hotkey": clipboard_key,
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
        FIGGHOST_PASTEBOARD="figghost-vision-check-" + name,
    )
    errors = os.path.join(root, name + ".err")
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=open(errors, "w"), stdout=subprocess.DEVNULL)
    running.append(process)
    return process, config, errors


def stop(process):
    process.terminate()
    try:
        process.wait(5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def cpu_seconds(process):
    out = subprocess.run(["ps", "-o", "cputime=", "-p", str(process.pid)], capture_output=True, text=True).stdout.strip()
    parts = out.replace(".", ":").split(":")
    nums = [int(p) for p in parts if p.isdigit()]
    # cputime prints [[h:]m:]s.cc
    cs = nums[-1] / 100.0
    secs = nums[-2] if len(nums) >= 2 else 0
    mins = nums[-3] if len(nums) >= 3 else 0
    return mins * 60 + secs + cs


def text(path):
    with open(path) as f:
        return f.read()


try:
    # 1. An enabled picker starts quietly, with its shortcut parsed and registered without an error.
    first, _, first_errors = start("one", {"enabled": True, "shortcut": SHORTCUT}, "f13")
    time.sleep(3)
    check(first.poll() is None, "an enabled picker starts and keeps running")
    check("colour picker" not in text(first_errors), "it starts without a complaint")

    # 2. Idle costs nothing: no timer runs while the picker is not in use.
    before = cpu_seconds(first)
    time.sleep(6)
    used = cpu_seconds(first) - before
    check(used < 0.06, "the idle program used %.2f s of CPU in 6 s (under 1 percent)" % used)
    stop(first)

    # 3. Off by default: with no vision key at all the program starts quietly too.
    off, _, off_errors = start("off", {}, "f13")
    time.sleep(2)
    check(off.poll() is None and "colour picker" not in text(off_errors), "a config without the feature starts quietly")
    stop(off)

    # 4. A removed shortcut is a valid state.
    none, _, none_errors = start("none", {"enabled": True, "shortcut": ""}, "f13")
    time.sleep(2)
    check(none.poll() is None and "colour picker" not in text(none_errors), "a picker without a shortcut starts quietly")
    stop(none)

    # 5. A hostile config is survived and not rewritten.
    bad, config, bad_errors = start("bad", {"enabled": True, "shortcut": "not a shortcut", "format": "rm -rf /"}, "f13")
    before = text(config)
    time.sleep(3)
    check(bad.poll() is None, "a config with a bad shortcut and format does not stop the program")
    check("invalid_shortcut" in text(bad_errors), "the bad shortcut is reported on stderr")
    check(text(config) == before, "the config file was not overwritten")
    stop(bad)
finally:
    for p in running:
        if p.poll() is None:
            stop(p)
    shutil.rmtree(root, ignore_errors=True)

if failures:
    print("%d checks failed" % len(failures))
    sys.exit(1)
print("all checks passed")
