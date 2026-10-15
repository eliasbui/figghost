"""End-to-end check of Smart Select through the real program, on a private instance.

Each case starts the program with its own data folder, the text to answer for (FIGGHOST_SMART_TEXT) and the
window placed outside every screen, so nothing appears on the screen and no key or pointer is used. The
answers are read from the window through the accessibility tree. No network is used: the exchange rates come
from a file in the data folder and translation is switched off.

    python3 -I scripts/check-smart.py target/debug/app-ui
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

BIN = sys.argv[1] if len(sys.argv) > 1 else sys.exit("usage: check-smart.py <path to the program>")

READ = """
function run(argv) {
  const proc = Application('System Events').processes.whose({ unixId: parseInt(argv[0]) })[0];
  const found = [];
  for (const window of proc.windows()) {
    const at = window.position();
    for (const element of window.entireContents()) {
      let role = '';
      try { role = element.role(); } catch (e) { continue; }
      if (role === 'AXStaticText') {
        found.push({ x: at[0], label: element.description(), value: String(element.value()) });
      }
    }
  }
  return JSON.stringify(found);
}
"""


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


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def rates_body():
    """A table shaped like the provider's, with a next update far away so that nothing is downloaded."""
    table = {"USD": 1, "EUR": 0.894425, "VND": 25890.201508, "JPY": 158.305497, "GBP": 0.756165}
    for i in range(30):
        table["X" + chr(65 + i // 26) + chr(65 + i % 26)] = i + 1
    return json.dumps(
        {
            "result": "success",
            "time_last_update_unix": 1792022551,
            "time_next_update_unix": 4102444800,
            "base_code": "USD",
            "rates": table,
        }
    )


def answer_for(text, language="en", config_extra=None, expect_window=True):
    """Starts the program for one text and returns (label, value) of the answer window, or (None, None). A run
    that finds no window is tried again, because another process on the machine can stop a test program."""
    for attempt in range(3 if expect_window else 1):
        result = answer_once(text, language, config_extra, 25 if expect_window else 8, expect_window)
        if result != (None, None):
            return result
    return (None, None)


def answer_once(text, language, config_extra, seconds, verbose):
    root = tempfile.mkdtemp(prefix="figghost-smart.")
    data = os.path.join(root, "data")
    os.makedirs(data)
    with open(os.path.join(data, "rates.json"), "w") as f:
        f.write(rates_body())
    config = {
        "language": language,
        "smart": dict({"enabled": True, "internet": False}, **(config_extra or {})),
        "show_settings_on_launch": False,
        "hotkey": "f13",
        "auto_paste": False,
        "tiling": {"enabled": False},
        "mouse": {"smooth": False},
    }
    with open(os.path.join(data, "config.json"), "w") as f:
        json.dump(config, f)
    env = dict(
        os.environ,
        FIGGHOST_HOME=data,
        FIGGHOST_PASTEBOARD="figghost-smart-check",
        FIGGHOST_NOTCH_OFFSCREEN="1",
        FIGGHOST_SMART_TEXT=text,
        FIGGHOST_SMART_OPEN="1",
    )
    errors = os.path.join(root, "stderr.log")
    process = subprocess.Popen([BIN, "--background"], env=env, stdout=subprocess.DEVNULL, stderr=open(errors, "w"))
    last = ""
    try:
        deadline = time.time() + seconds
        while time.time() < deadline:
            time.sleep(0.4)
            if process.poll() is not None:
                last = f"the program exited with {process.returncode}"
                break
            done = subprocess.run(
                ["osascript", "-l", "JavaScript", "-e", READ, str(process.pid)], capture_output=True, timeout=60
            )
            # The program is not known to System Events for a moment after it starts: ask again.
            if done.returncode != 0:
                last = done.stderr.decode().strip()
                continue
            last = "no answer window in the accessibility tree: " + done.stdout.decode()[:200]
            # The answer window opens at x = -30000; the notch is somewhere else outside the screen.
            found = [item for item in json.loads(done.stdout.decode() or "[]") if abs(item["x"] + 30000) < 5]
            if found:
                return (found[0]["label"], found[0]["value"])
        if verbose:
            print("      no answer window: " + last)
            with open(errors) as f:
                print("      program output: " + f.read()[-300:].replace("\n", " | "))
        return (None, None)
    finally:
        process.kill()
        process.wait()
        shutil.rmtree(root, ignore_errors=True)


if screen_is_locked():
    sys.exit("smart e2e skipped: the screen is locked, so the answer window cannot be read; run it again when it is not")

try:
    label, value = answer_for("5 km")
    check(label == "5 km", f"the header says what was read ({label!r})")
    check(value == "3.107 mi, Length\n16,404 ft, Length", f"a length in miles and feet ({value!r})")

    label, value = answer_for("1,5 km", language="vi")
    check(label == "1,5 km", "a decimal comma is read in Vietnamese")
    check(value == "4.921 ft, Chi\u1ec1u d\u00e0i\n0,9321 mi, Chi\u1ec1u d\u00e0i", f"and answered in Vietnamese style ({value!r})")

    label, value = answer_for("72 F")
    check(value.startswith("22.22 \u00b0C, Temperature"), f"a temperature ({value!r})")

    label, value = answer_for("$100")
    check(label == "100.00 USD", f"an amount of money ({label!r})")
    check(
        value == "89.44 EUR, 1 USD = 0.8944 EUR\n2,589,020 VND, 1 USD = 25,890 VND",
        f"is converted with the rates in the data folder ({value!r})",
    )

    label, value = answer_for("5tr \u0111", language="vi")
    check(label == "5.000.000 VND", f"Vietnamese shorthand for money ({label!r})")
    check(value.split("\n")[0].startswith("193,12 USD"), f"is converted too ({value!r})")

    label, value = answer_for("3pm PST")
    check(label == "3:00 PM PST", f"a time with a zone ({label!r})")
    check("11:00 PM, UTC" in value.split("\n"), f"shown in UTC ({value!r})")
    check(any("Tokyo" in line for line in value.split("\n")), "and in Tokyo")

    label, value = answer_for("good morning")
    check(
        "Turn on the internet" in value,
        f"words are not sent anywhere while the internet is off ({value!r})",
    )

    label, value = answer_for("$100", config_extra={"currencies": ["JPY", "GBP"]})
    check("JPY" in value and "GBP" in value and "EUR" not in value, f"the currencies come from the settings ({value!r})")

    result = answer_for("5 km", config_extra={"enabled": False}, expect_window=False)
    check(result == (None, None), "nothing opens while the feature is off")
finally:
    pass

print("smart e2e ok" if not failures else f"smart e2e broken: {len(failures)} failure(s)")
sys.exit(1 if failures else 0)
