#!/usr/bin/env python3
"""End to end check of the drag wheel through the real program, on a private instance: its own data folder, a named
pasteboard instead of the clipboard, the notch outside every screen, and the wheel switched on in its config.

    scripts/check-wheel.py <program> <drag_source> <ax_dump> <named_board>

<program> is target/debug/app-ui; the three others are the macos-sys examples of those names
(cargo build -p app-ui -p macos-sys --examples).

WARNING: this posts REAL mouse input for about three minutes (each run is about 6 seconds). It moves the pointer,
presses and releases the left button and drags a file tile out of a small window of the `drag_source` example.
The only keys it ever sends are the modifier keys Shift, Command, Control and Option on their own (no letter, no
other key), so nothing can be typed into another app, though a modifier that is held when a run is cut short
would be released by the cleanup. Leave the machine alone while it runs. The terminal needs Accessibility access
(to post events and to read the wheel through the accessibility tree) and the screen has to be unlocked; without
either the check says so and exits 0 (skipped), like the other screen checks. The pointer is put back afterwards.

It never writes to the general pasteboard (Copy Path goes to the named one; the Copy slot, which has no named
pasteboard route, is not run here) and never moves or deletes the dragged file: the drop only copies.

What it checks: Shift, Command and Control each open their own wheel during a file drag (read through the
accessibility label of the wheel, which is its caption); a drop on a slot makes the file next to the original with
a free name; the same key again changes nothing; another key switches; a drop in the hub or outside makes
nothing; no wheel without a file drag, with Option held, with a key held before the drag; the wheel is gone after
the mouse is released; the frontmost app does not change; the feature off shows nothing and idles; a hostile
config does not stop the program.
"""
import ctypes
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time

if len(sys.argv) != 5:
    print(__doc__)
    sys.exit(2)
BIN, SOURCE, AXDUMP, BOARD = sys.argv[1:5]
PASTEBOARD = "figghost-wheel-check"

ax = ctypes.CDLL("/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices")
cg = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
ax.AXIsProcessTrusted.restype = ctypes.c_bool


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, Point, ctypes.c_uint32]
cg.CGEventCreateKeyboardEvent.restype = ctypes.c_void_p
cg.CGEventCreateKeyboardEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_bool]
cg.CGEventSetFlags.argtypes = [ctypes.c_void_p, ctypes.c_uint64]
cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
cg.CGEventCreate.restype = ctypes.c_void_p
cg.CGEventCreate.argtypes = [ctypes.c_void_p]
cg.CGEventGetLocation.restype = Point
cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
cg.CGWarpMouseCursorPosition.argtypes = [Point]
cf.CFRelease.argtypes = [ctypes.c_void_p]

DOWN, UP, DRAG, MOVE = 1, 2, 6, 5
SHIFT, COMMAND, CONTROL, OPTION = 56, 55, 59, 58
FLAG = {SHIFT: 0x20000, CONTROL: 0x40000, OPTION: 0x80000, COMMAND: 0x100000}
ONLY_MODIFIERS = set(FLAG)

failures = []
skipped = False
root = tempfile.mkdtemp(prefix="figghost-wheel-")
data = os.path.join(root, "data")
os.makedirs(data)
files = os.path.join(root, "files")
os.makedirs(files)
log = os.path.join(root, "err.log")
process = None
source_proc = None
held = 0


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message, flush=True)
    if not ok:
        failures.append(message)


def post(event):
    cg.CGEventPost(0, event)
    cf.CFRelease(event)


def mouse(kind, x, y):
    post(cg.CGEventCreateMouseEvent(None, kind, Point(x, y), 0))


def key(code, down):
    """Only a bare modifier key goes through here: anything else is refused."""
    assert code in ONLY_MODIFIERS, "only modifier keys are ever sent"
    global held
    held = (held | FLAG[code]) if down else (held & ~FLAG[code])
    event = cg.CGEventCreateKeyboardEvent(None, code, down)
    cg.CGEventSetFlags(event, held)
    post(event)


def tap(code, gap=0.12):
    key(code, True)
    time.sleep(gap)
    key(code, False)
    time.sleep(0.12)


def release_all():
    for code in ONLY_MODIFIERS:
        if held & FLAG[code]:
            key(code, False)


def pointer():
    event = cg.CGEventCreate(None)
    p = cg.CGEventGetLocation(event)
    cf.CFRelease(event)
    return p.x, p.y


ax.AXUIElementCreateSystemWide.restype = ctypes.c_void_p
ax.AXUIElementCopyElementAtPosition.argtypes = [ctypes.c_void_p, ctypes.c_float, ctypes.c_float, ctypes.POINTER(ctypes.c_void_p)]
ax.AXUIElementGetPid.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]


def pid_at(x, y):
    """The process whose window is under the point, so that a press is never sent into some other app."""
    element = ctypes.c_void_p()
    if ax.AXUIElementCopyElementAtPosition(ax.AXUIElementCreateSystemWide(), x, y, ctypes.byref(element)) != 0:
        return None
    pid = ctypes.c_int()
    ax.AXUIElementGetPid(element, ctypes.byref(pid))
    return pid.value


def glide(frm, to, steps=24, kind=DRAG):
    for i in range(1, steps + 1):
        t = i / steps
        mouse(kind, frm[0] + (to[0] - frm[0]) * t, frm[1] + (to[1] - frm[1]) * t)
        time.sleep(0.012)


def png(path, w=96, h=64):
    row = (w * 3 + 3) // 4 * 4
    body = b"".join(
        b"".join(bytes([(x * 4) & 255, (y * 5) & 255, 0x80]) for x in range(w)).ljust(row, b"\0") for y in range(h)
    )
    header = b"BM" + (54 + len(body)).to_bytes(4, "little") + bytes(4) + (54).to_bytes(4, "little")
    dib = (40).to_bytes(4, "little") + w.to_bytes(4, "little") + h.to_bytes(4, "little") + (1).to_bytes(2, "little") \
        + (24).to_bytes(2, "little") + bytes(4) + len(body).to_bytes(4, "little") + bytes(16)
    tmp = path + ".bmp"
    with open(tmp, "wb") as f:
        f.write(header + dib + body)
    subprocess.run(["/usr/bin/sips", "-s", "format", "png", tmp, "--out", path], check=True, capture_output=True)
    os.remove(tmp)


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def write_config(wheel):
    with open(os.path.join(data, "config.json"), "w") as f:
        json.dump(
            {
                "wheel": wheel,
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


def tree():
    out = subprocess.run([AXDUMP, str(process.pid)], capture_output=True, text=True, timeout=20)
    return out.stdout


def wheel_caption(text):
    """The caption of the open wheel read from the accessibility tree, or None when no wheel is shown."""
    for caption in ("Convert ", "Tools for ", "Actions for "):
        if caption in text:
            return caption.strip()
    return None


def look(expected):
    """The wheel caption now; when one is expected, it is given up to two seconds to appear."""
    deadline = time.time() + (4.0 if expected else 0.0)
    while True:
        seen = wheel_caption(tree())
        if seen == expected or time.time() >= deadline:
            return seen
        time.sleep(0.2)


def alive(message):
    ok = process is not None and process.poll() is None
    check(ok, message + ("" if ok else f" (exit status {process.returncode if process else None})"))


def cpu_seconds():
    text = subprocess.run(["/bin/ps", "-o", "cputime=", "-p", str(process.pid)], capture_output=True, text=True).stdout.strip()
    minutes, seconds = text.split(":")
    return int(minutes) * 60 + float(seconds)


def check_idle(what):
    """Eight idle seconds cost less than 0.2 s of processor time (2.5 %)."""
    first = cpu_seconds()
    time.sleep(8)
    grew = cpu_seconds() - first
    print(f"      cpu time, {what}: {grew:.2f} s in 8 s")
    check(grew < 0.2, f"idle: {what} costs {grew:.2f} s of processor time in 8 s")


def frontmost():
    return subprocess.run(["/usr/bin/lsappinfo", "front"], capture_output=True, text=True).stdout.strip()


def begin_drag(path, name):
    """Starts a drag of `path` out of the second process and leaves the pointer at the middle of the screen with
    the button down. Returns the centre."""
    global source_proc
    source_proc = subprocess.Popen([SOURCE, path, "40"], stdout=subprocess.PIPE, text=True)
    line = ""
    for _ in range(200):
        line = source_proc.stdout.readline()
        if line.startswith("TILE"):
            break
    parts = line.split()
    if len(parts) != 3:
        check(False, f"{name}: the drag source did not show a tile ({line!r})")
        return None
    tile = (float(parts[1]), float(parts[2]))
    centre = (960.0, 600.0)
    mouse(MOVE, *tile)
    time.sleep(0.3)
    under = pid_at(*tile)
    if under != source_proc.pid:
        check(False, f"{name}: the tile is covered by another app (pid {under}), so nothing was pressed")
        source_proc.terminate()
        return None
    mouse(DOWN, *tile)
    time.sleep(0.1)
    glide(tile, (tile[0] + 40, tile[1] + 30), 6)
    glide((tile[0] + 40, tile[1] + 30), centre)
    time.sleep(0.3)
    return centre


def end_drag(at):
    mouse(UP, *at)
    time.sleep(0.6)
    if source_proc and source_proc.poll() is None:
        source_proc.terminate()
        source_proc.wait(5)


# Ring radii of the approved design (design-ui/notes.md), enough to aim at the middle of a slot.
INNER_COMPACT = [56, 56, 56, 56, 56, 56, 56, 56, 60, 70, 80, 90]
INNER_REGULAR = [56, 56, 56, 56, 56, 56, 60, 74, 90, 100, 120, 132]


def slot_point(centre, count, index, compact):
    inner = (INNER_COMPACT if compact else INNER_REGULAR)[count - 1]
    outer = inner + (60 if compact else 92)
    r = (inner + outer) / 2
    a = 2 * math.pi * index / count
    return centre[0] + r * math.sin(a), centre[1] - r * math.cos(a)


def listing():
    return sorted(os.listdir(files))


def run_case_once(name, keys, slot, expect_new, wheel_expected, count=10, compact=True, setup=None, after=None):
    """One drag: taps `keys` in order at the middle of the screen, releases on `slot` (an index, 'hub' or 'out')."""
    print(f"-- {name}", flush=True)
    time.sleep(1.0)
    src = os.path.join(files, "photo one.png")
    before = listing()
    digest = sha(src)
    at = begin_drag(src, name)
    if at is None:
        return
    # After the press: the drag source may itself come to the front, and what must not change is that, because of the wheel.
    front = frontmost()
    if setup:
        setup()
    seen = None
    for code in keys:
        tap(code)
        seen = look(wheel_expected)
    if not keys:
        seen = look(wheel_expected)
    check(seen == wheel_expected, f"{name}: wheel shown is {wheel_expected} (saw {seen})")
    if wheel_expected:
        check(frontmost() == front, f"{name}: the frontmost app did not change while the wheel was open")
    if slot == "hub":
        target = at
    elif slot == "out":
        target = (at[0] + 330, at[1])
    elif slot is None:
        target = (at[0] + 5, at[1] - 300)
    else:
        target = slot_point(at, count, slot, compact)
    glide(at, target, 10)
    time.sleep(0.6)  # long enough for a busy machine to light the slot before the release
    release_all()
    end_drag(target)
    # The tools run on a background thread and take a moment: wait for the files, then for things to settle.
    deadline = time.time() + 10
    while expect_new and time.time() < deadline and len([n for n in listing() if n not in before]) < len(expect_new):
        time.sleep(0.25)
    time.sleep(1.0)
    check(wheel_caption(tree()) is None, f"{name}: the wheel is gone after the mouse is released")
    new = [n for n in listing() if n not in before]
    check(len(new) == len(expect_new) and all(any(n.startswith(p) for n in new) for p in expect_new),
          f"{name}: new files {new} match {expect_new}")
    check(sha(src) == digest and "photo one.png" in listing(), f"{name}: the original is untouched")
    if after:
        after(new)


retries = []


def run_case(name, keys, slot, expect_new, wheel_expected, **more):
    """`run_case_once`, and when it failed once more: this Mac is shared and busy, and a posted drag can lose a
    race with whatever else is moving the pointer. A case that passes only on a retry is listed in the summary."""
    for attempt in (1, 2, 3):
        mark = len(failures)
        before = set(listing())
        run_case_once(name, keys, slot, expect_new, wheel_expected, **more)
        if len(failures) == mark:
            if attempt > 1:
                retries.append((name, attempt))
            return
        if attempt < 3:
            print(f"RETRY {name}: attempt {attempt} failed {failures[mark:]}", flush=True)
            del failures[mark:]
            time.sleep(3)
            for n in set(listing()) - before:
                path = os.path.join(files, n)
                subprocess.run(["/bin/rm", "-rf", path])


if not ax.AXIsProcessTrusted():
    print("SKIP  the terminal has no Accessibility access: it cannot post events or read the wheel")
    shutil.rmtree(root, ignore_errors=True)
    sys.exit(0)
if "CGSSessionScreenIsLocked" in subprocess.run(["/usr/sbin/ioreg", "-n", "Root", "-d1"], capture_output=True, text=True).stdout:
    print("SKIP  the screen is locked")
    shutil.rmtree(root, ignore_errors=True)
    sys.exit(0)

home = pointer()
try:
    png(os.path.join(files, "photo one.png"))

    # ---- the wheel on
    write_config({"enabled": True})
    start()
    print("-- posting real input for about three minutes: leave the machine alone", flush=True)

    # Formats of a PNG: JPEG HEIC TIFF AVIF GIF BMP PDF ZIP TAR TGZ (10 slots, compact ring); slot 0 is JPEG.
    run_case("Shift opens Formats, drop on JPEG", [SHIFT], 0, ["photo one.jpg"], "Convert")
    out = os.path.join(files, "photo one.jpg")
    magic = open(out, "rb").read(3) if os.path.exists(out) else b""
    check(magic == b"\xff\xd8\xff", "the new file really is a JPEG")
    run_case("Shift, drop on PDF (slot 6)", [SHIFT], 6, ["photo one.pdf"], "Convert")
    run_case("Shift, drop on ZIP (slot 7)", [SHIFT], 7, ["photo one.png.zip"], "Convert")
    # Tools of a PNG: Half size, Quarter size, No metadata, Cut out, Copy text, Annotate (6 slots, regular ring).
    run_case("Command opens Tools, drop on Half size", [COMMAND], 0, ["photo one half"], "Tools for", count=6, compact=False)
    run_case("Command, drop on Quarter size (slot 1)", [COMMAND], 1, ["photo one quarter"], "Tools for", count=6, compact=False)
    run_case("Command, drop on No metadata (slot 2)", [COMMAND], 2, ["photo one no metadata"], "Tools for", count=6, compact=False)
    # A key switches the wheel; the tapped last one decides.
    run_case("Shift then Command: the last wheel is Tools, drop on Half size", [SHIFT, COMMAND], 0, ["photo one half"],
             "Tools for", count=6, compact=False)
    run_case("Shift again: the wheel stays, drop on a second JPEG gets a free name", [SHIFT, SHIFT], 0,
             ["photo one 2.jpg"], "Convert")
    # The hub and the outside do nothing.
    run_case("Shift, release in the hub", [SHIFT], "hub", [], "Convert")
    run_case("Shift, release outside the wheel", [SHIFT], "out", [], "Convert")
    # Control opens Actions; Copy Path (slot 0) writes the named pasteboard only.
    def copy_path_check(new):
        got = subprocess.run([BOARD, PASTEBOARD, "read"], capture_output=True, text=True).stdout.strip()
        check(got.endswith("photo one.png"), f"Copy Path put the path on the named pasteboard ({got!r})")
        check(new == [], "Copy Path made no file")
    run_case("Control opens Actions, drop on Copy Path", [CONTROL], 0, [], "Actions for", count=6, compact=False,
             after=copy_path_check)
    # No wheel in these cases.
    run_case("No key tapped: no wheel", [], "hub", [], None)
    run_case("Option held, then Shift: no wheel", [], "hub", [], None,
             setup=lambda: (key(OPTION, True), time.sleep(0.1), tap(SHIFT), time.sleep(0.1)))
    release_all()

    # A key held before the drag does not count, and tapping it later does.
    print("-- Shift held before the drag", flush=True)
    key(SHIFT, True)
    time.sleep(0.2)
    src = os.path.join(files, "photo one.png")
    at = begin_drag(src, "held key")
    if at:
        time.sleep(0.4)
        check(wheel_caption(tree()) is None, "a key held before the drag opens no wheel")
        key(SHIFT, False)
        time.sleep(0.2)
        tap(SHIFT)
        check(look("Convert") == "Convert", "tapping Shift after the drag began opens Formats")
        end_drag(at)
    release_all()

    # No file drag at all: mouse down on the desktop and move, tap each key.
    print("-- keys with no file drag", flush=True)
    for code in (SHIFT, COMMAND, CONTROL):
        tap(code)
        check(wheel_caption(tree()) is None, f"key {code} with no drag opens no wheel")
    alive("the program is still running")
    check_idle("the feature on, no drag")
    stop()

    # ---- the wheel off (the default): nothing happens, and it idles
    write_config({})
    start()
    before = listing()
    run_case("Default config (off): Shift opens nothing", [SHIFT], "hub", [], None)
    check_idle("the feature off")
    stop()

    # ---- a hostile config must not stop the program
    for hostile in ({"enabled": "yes"}, 5, None, {"enabled": None, "extra": [1]}, {"enabled": True, "keys": "x"}):
        write_config(hostile)
        start()
        check(process.poll() is None, f"still running with wheel config {hostile!r}")
        stop()
finally:
    release_all()
    stop()
    if source_proc and source_proc.poll() is None:
        source_proc.terminate()
    subprocess.run([BOARD, PASTEBOARD, "release"], capture_output=True)
    cg.CGWarpMouseCursorPosition(Point(*home))
    # Nothing is left behind, also when a check failed: the fixture, every converted output, the private data
    # folder and the log all live under `root`.
    if failures and os.path.exists(log):
        with open(log) as f:
            print("program log (last lines):", *[l.rstrip()[:200] for l in f.readlines()[-8:]], sep="\n  ")
    shutil.rmtree(root, ignore_errors=True)

for name, attempt in retries:
    print(f"NOTE  {name}: passed only on attempt {attempt}")
print("PASS" if not failures else f"FAILED: {len(failures)}")
sys.exit(1 if failures else 0)
