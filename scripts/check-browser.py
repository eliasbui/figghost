"""Plays a browser against the real host program and a private instance of the app.

Usage: python3 -I scripts/check-browser.py <program>   (for example target/debug/app-ui)
Starts the app on a private data directory with the Browser feature on and Video download off, then starts the program
as a browser would, with the extension's origin, and speaks native messaging to it.
"""
import json, os, shutil, struct, subprocess, sys, tempfile, time

ORIGIN = "chrome-extension://gaolignoeegofacmdaebpionmopdfpfn/"
program = sys.argv[1]
work = tempfile.mkdtemp(prefix="figghost-bridge-")
home = os.path.join(work, "home")
os.makedirs(home)
with open(os.path.join(home, "config.json"), "w") as f:
    json.dump({"show_settings_on_launch": False,
               "browser": {"enabled": True, "sites": ["youtube.com", "https://www.Facebook.com/x", "nonsense"]},
               "download": {"enabled": False}}, f)
env = dict(os.environ, FIGGHOST_HOME=home, FIGGHOST_NOTCH_OFFSCREEN="1", FIGGHOST_TRACE="1")
failed = 0


def check(ok, what):
    global failed
    print(("ok     " if ok else "FAILED ") + what)
    failed |= not ok


def send(proc, message):
    body = json.dumps(message).encode()
    proc.stdin.write(struct.pack("=I", len(body)) + body)
    proc.stdin.flush()


def read(proc):
    head = proc.stdout.read(4)
    if len(head) < 4:
        return None
    (length,) = struct.unpack("=I", head)
    return json.loads(proc.stdout.read(length))


app = subprocess.Popen([program, "--background"], env=env, stderr=open(os.path.join(work, "app.log"), "w"))
try:
    time.sleep(4)
    check(app.poll() is None, "the app runs with the Browser feature on")
    check(os.path.exists(os.path.join(home, "browser.sock")), "it opened its socket in the data directory")
    mode = os.stat(os.path.join(home, "browser.sock")).st_mode & 0o777
    check(mode == 0o600, f"the socket is this user's alone ({oct(mode)})")

    wrong = subprocess.run([program, "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"], env=env,
                           input=b"", capture_output=True, timeout=10)
    check(wrong.returncode == 2 and wrong.stdout == b"", "another extension's origin gets nothing")

    host = subprocess.Popen([program, ORIGIN], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    send(host, {"type": "hello"})
    state = read(host)
    check(state == {"type": "state", "block": False, "sites": ["youtube.com", "facebook.com"], "app": True},
          f"hello is answered with what to block: {state}")
    send(host, {"type": "download", "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ"})
    reply = read(host)
    check(reply == {"type": "download", "ok": False, "error": "download_off"},
          f"an address reaches the app, which says why it cannot download it: {reply}")
    send(host, {"type": "download", "url": "file:///etc/passwd"})
    reply = read(host)
    check(reply == {"type": "download", "ok": False, "error": "bad_url"}, f"an address that is not of the web is refused: {reply}")
    host.stdin.close()
    host.wait(timeout=10)
    check(host.returncode == 0, "the host ends when the browser closes its end")

    app.terminate()
    app.wait(timeout=10)
    host = subprocess.Popen([program, ORIGIN], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    send(host, {"type": "hello"})
    state = read(host)
    check(state == {"type": "state", "block": False, "sites": [], "app": False}, f"without the app nothing is blocked: {state}")
    host.stdin.close()
    host.wait(timeout=10)
    log = open(os.path.join(work, "app.log")).read()
    check("panicked" not in log, "nothing panicked")
finally:
    if app.poll() is None:
        app.kill()
    shutil.rmtree(work)
sys.exit(failed)
