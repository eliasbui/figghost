"""End-to-end check of the clipboard extras (boards schema, sensitive data) through the real program, on a
private instance.

Starts the program with its own data folder, a named pasteboard instead of the real clipboard and a notch
placed outside every screen, so it never touches the real settings, the real history or the screen. It
makes a database shaped like the previous version's, lets the program upgrade it, plays an app that copies
(with `named_board`), and reads what the program stored. It posts no input events, so a double press of a
modifier key and a real paste are not part of this check.

    python3 -I scripts/check-stack.py target/debug/app-ui target/debug/examples/named_board
"""

import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

if len(sys.argv) < 3:
    sys.exit("usage: check-stack.py <the program> <named_board example>")
BIN, BOARD = sys.argv[1], sys.argv[2]
PASTEBOARD = "figghost-stack-check"
SECRET = "my card is 4111 1111 1111 1111 thanks"
PLAIN = "meeting notes about the quarterly report"
OTHER_SECRET = "password: hunter2xyz"

root = tempfile.mkdtemp(prefix="figghost-stack.")
data = os.path.join(root, "data")
os.makedirs(data)
db_path = os.path.join(data, "history.sqlite")
failures = []
process = None

V2_SCHEMA = """
CREATE TABLE item (
    id INTEGER PRIMARY KEY, kind TEXT NOT NULL, hash INTEGER NOT NULL, text TEXT NOT NULL,
    fold TEXT NOT NULL, image TEXT, width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0,
    app_id TEXT, app_name TEXT, used_at INTEGER NOT NULL, uses INTEGER NOT NULL DEFAULT 1,
    pinned INTEGER NOT NULL DEFAULT 0, pin_key TEXT
);
CREATE TABLE rich (item_id INTEGER PRIMARY KEY, rtf BLOB, html BLOB);
CREATE TRIGGER item_rich_ad AFTER DELETE ON item BEGIN DELETE FROM rich WHERE item_id = old.id; END;
CREATE UNIQUE INDEX item_dedupe ON item(kind, hash);
CREATE INDEX item_recent ON item(pinned DESC, used_at DESC);
CREATE VIRTUAL TABLE item_fts USING fts5(fold, content='item', content_rowid='id', tokenize='trigram');
CREATE TRIGGER item_ai AFTER INSERT ON item BEGIN INSERT INTO item_fts(rowid, fold) VALUES (new.id, new.fold); END;
CREATE TRIGGER item_ad AFTER DELETE ON item BEGIN INSERT INTO item_fts(item_fts, rowid, fold) VALUES ('delete', old.id, old.fold); END;
PRAGMA user_version = 2;
"""


def check(ok, message):
    print(("ok    " if ok else "FAIL  ") + message)
    if not ok:
        failures.append(message)


def write_config(sensitive, minutes):
    with open(os.path.join(data, "config.json"), "w") as f:
        json.dump(
            {
                "stack": {"enabled": True, "modifier": "option", "sensitive": sensitive, "expire_minutes": minutes},
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
)


def start():
    global process
    process = subprocess.Popen([BIN, "--background"], env=env, stderr=open(os.path.join(root, "err.log"), "a"), stdout=subprocess.DEVNULL)
    time.sleep(1.5)
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
    subprocess.run([BOARD, PASTEBOARD, "write", text], check=True)
    # The program looks at the pasteboard about every 0.4 s.
    time.sleep(1.0)


def rows(query="SELECT id, text, masked, fold, expires_at, pinned FROM item ORDER BY id"):
    con = sqlite3.connect("file:%s?mode=ro" % db_path, uri=True)
    try:
        return con.execute(query).fetchall()
    finally:
        con.close()


def cpu_of(pid, seconds):
    """Average CPU use of a process over a few seconds, from its accumulated CPU time."""

    def used():
        out = subprocess.run(["ps", "-o", "cputime=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
        parts = out.replace(".", ":").split(":")
        h = m = s = 0
        cs = 0
        if len(parts) == 4:
            h, m, s, cs = (int(p) for p in parts)
        elif len(parts) == 3:
            m, s, cs = (int(p) for p in parts)
        return h * 3600 + m * 60 + s + cs / 100.0

    before = used()
    time.sleep(seconds)
    return (used() - before) / seconds * 100.0


try:
    # 1. A database made by the previous version is upgraded without losing anything, after a copy is kept.
    con = sqlite3.connect(db_path)
    con.executescript(V2_SCHEMA)
    for i, (text, pinned, key) in enumerate([("old pinned note", 1, "b"), ("old plain note about invoices", 0, None)]):
        con.execute(
            "INSERT INTO item (kind, hash, text, fold, used_at, pinned, pin_key) VALUES ('text', ?, ?, ?, ?, ?, ?)",
            (i + 100, text, text, 1000 + i, pinned, key),
        )
    con.commit()
    con.close()
    original = open(db_path, "rb").read()
    write_config("mask", 1)
    start()
    backup = os.path.join(data, "history.sqlite.v2.bak")
    check(os.path.exists(backup), "a copy of the old database was kept")
    check(os.path.exists(backup) and open(backup, "rb").read() == original, "the copy is the old file, byte for byte")
    check(rows("PRAGMA user_version")[0][0] == 3, "the database is at version 3")
    check(os.path.exists(backup) and (os.stat(backup).st_mode & 0o777) == 0o600, "the copy is private to its owner")
    old = rows()
    check([r[1] for r in old][:2] == ["old pinned note", "old plain note about invoices"], "the old items are all there")
    check(old[0][5] == 1 and rows("SELECT pin_key FROM item WHERE id = 1")[0][0] == "b", "the old pin and its letter survived")
    check(rows("SELECT count(*) FROM item_fts WHERE item_fts MATCH 'invoices'")[0][0] == 1, "the old rows are still in the search index")

    # 2. Copies: a plain one is stored as it was, a likely secret is stored hidden.
    copy(PLAIN)
    copy(SECRET)
    stored = {r[1]: r for r in rows()}
    check(PLAIN in stored and stored[PLAIN][2] == 0, "a plain copy is stored normally")
    check(SECRET in stored and stored[SECRET][2] == 1, "a card number is stored masked")
    check(SECRET in stored and stored[SECRET][3] == "", "a masked item is not in the search text")
    check(
        rows("SELECT count(*) FROM item_fts WHERE item_fts MATCH '4111'")[0][0] == 0,
        "its digits are not in the search index",
    )
    check(
        SECRET in stored and stored[SECRET][4] is not None and stored[SECRET][4] > int(time.time() * 1000),
        "it has a time to go",
    )
    check(PLAIN in stored and stored[PLAIN][4] is None, "a plain copy has none")

    # 3. Idle cost with the feature on and a keyboard monitor installed.
    cpu = cpu_of(process.pid, 6)
    check(cpu < 1.0, "idle CPU stays under 1 percent (%.2f%%)" % cpu)

    # 4. The masked item goes away once its minute is up; everything else stays.
    deadline = time.time() + 100
    while time.time() < deadline and SECRET in {r[1] for r in rows()}:
        time.sleep(2)
    left = {r[1] for r in rows()}
    check(SECRET not in left, "the masked item was deleted after its time")
    check({PLAIN, "old pinned note", "old plain note about invoices"} <= left, "the other items stayed")
    check(process.poll() is None, "the program kept running")
    stop()

    # 5. With 'never keep', a secret is not stored at all.
    write_config("never", 10)
    start()
    copy(OTHER_SECRET)
    copy("a harmless line of text")
    left = {r[1] for r in rows()}
    check(OTHER_SECRET not in left, "a secret is not stored with the 'never' choice")
    check("a harmless line of text" in left, "other copies still are")
    stop()

    # 6. With nothing chosen, everything is stored as before.
    write_config("off", 10)
    start()
    copy(OTHER_SECRET)
    stored = {r[1]: r for r in rows()}
    check(OTHER_SECRET in stored and stored[OTHER_SECRET][2] == 0, "with the default choice a secret is stored like any copy")
    stop()
finally:
    stop()
    subprocess.run([BOARD, PASTEBOARD, "release"], check=False)
    shutil.rmtree(root, ignore_errors=True)

if failures:
    print("\n%d check(s) failed" % len(failures))
    sys.exit(1)
print("\nstack: all checks passed")
