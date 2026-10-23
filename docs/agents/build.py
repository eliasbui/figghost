#!/usr/bin/env python3
"""Pixel sprites of the coding agents, which the closed notch shows while they work, ask, finish or fail
(crates/app-ui/src/agents.rs).

  build.py cut <sheet.png> <codex|pi|agy>   cut a generated 4 x 3 sheet on magenta into ART/<name>.png
  build.py claude                       draw ART/claude.png from the glyphs Claude Code draws Clawd with
  build.py check                        self-test

ART/<name>.png is a row of square frames, one image pixel per art pixel, two for each of POSES in that
order. A generated sheet holds the same ten frames in reading order, then two (standing, blinking) that are
left out. The cutting is the one of docs/mascots/build.py, with a palette for each agent.
"""
import importlib.util, pathlib, struct, sys, tempfile, zlib

HERE = pathlib.Path(__file__).resolve().parent
ART = HERE.parent.parent / "crates" / "app-ui" / "assets" / "agents"
_spec = importlib.util.spec_from_file_location("mascots", HERE.parent / "mascots" / "build.py")
mascots = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(mascots)

POSES = ["typing", "plowing", "asking", "done", "failed"]
FRAMES = 2 * len(POSES)


def rgb(hex_colour):
    return tuple(int(hex_colour[i:i + 2], 16) for i in (1, 3, 5))


# The colours of the mascots, what the generated sheets need beyond them (tears, a laptop, soil), and those
# of each logo: the bands of the Codex mark, the blocks of pi.dev/logo.svg and the four colours of Antigravity's arch.
BASE = mascots.PALETTE + ["#88E4F8", "#808080", "#6E3A1C"]
BRAND = {"codex": ["#B1A7FF", "#7A9DFF", "#3941FF"], "pi": ["#F09082", "#4D9ABF", "#F1BE58"],
         "agy": ["#4285F4", "#EA4335", "#FBBC05", "#34A853"]}


def write_rgba(path, rows):
    """rows of (r, g, b), or None where clear."""
    raw = b"".join(b"\x00" + b"".join(bytes(c + (255,)) if c else b"\x00" * 4 for c in row) for row in rows)
    chunk = lambda k, d: struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d))
    pathlib.Path(path).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(
        ">IIBBBBB", len(rows[0]), len(rows), 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def pack(frames, path):
    """Writes frames ({(x, y): colour} around one origin) as a row of square cells that all keep that
    origin in the same place, so the next frame moves only what moved."""
    xs = [x for f in frames for x, _ in f]
    ys = [y for f in frames for _, y in f]
    w, h = max(xs) - min(xs) + 1, max(ys) - min(ys) + 1
    side = max(w, h)
    left, top = min(xs) - (side - w) // 2, min(ys) - (side - h) // 2
    rows = [[None] * (side * len(frames)) for _ in range(side)]
    for k, frame in enumerate(frames):
        for (x, y), c in frame.items():
            rows[y - top][k * side + x - left] = c
    path.parent.mkdir(parents=True, exist_ok=True)
    write_rgba(path, rows)
    print(f"wrote {path} ({len(frames)} frames of {side} x {side})")


def label(px, palette):
    """Index in the palette, or -1 for the magenta around the sprites and the shadows it darkens into."""
    r, g, b, a = px
    if a < 128 or (r - g > 60 and b - g > 60 and abs(r - b) < 90):
        return -1
    return min(range(len(palette)), key=lambda i: sum((c - v) ** 2 for c, v in zip(palette[i], (r, g, b))))


def cut(sheet, name, art=ART):
    palette = [rgb(c) for c in BASE + BRAND[name]]
    w, h, px = mascots.read_png(sheet)
    seen = {}
    lab = [[seen[p] if p in seen else seen.setdefault(p, label(p, palette)) for p in row] for row in px]
    xs = [x for row in lab for x in range(1, w) if row[x] != row[x - 1]]
    ys = [y for x in range(w) for y in range(1, h) if lab[y][x] != lab[y - 1][x]]
    ux, ox = mascots.block_grid(xs, w / 4)
    uy, oy = mascots.block_grid(ys, h / 3)
    print(f"one art pixel is {ux:.2f} x {uy:.2f} image pixels")

    def sample(cx, cy):
        votes = {}
        for y in range(int(cy - uy * 0.2), int(cy + uy * 0.2) + 1):
            for x in range(int(cx - ux * 0.2), int(cx + ux * 0.2) + 1):
                if 0 <= x < w and 0 <= y < h:
                    votes[lab[y][x]] = votes.get(lab[y][x], 0) + 1
        return max(votes, key=votes.get) if votes else -1

    gw, gh = int((w - ox) / ux), int((h - oy) / uy)
    grid = [[sample(ox + (i + 0.5) * ux, oy + (j + 0.5) * uy) for i in range(gw)] for j in range(gh)]
    frames = []
    for k in range(FRAMES):
        c, r = k % 4, k // 4
        x0, x1, y0, y1 = gw * c // 4, gw * (c + 1) // 4, gh * r // 3, gh * (r + 1) // 3
        pts = mascots.clean({(x - x0, y - y0): grid[y][x] for y in range(y0, y1) for x in range(x0, x1)
                             if grid[y][x] >= 0}, min(x1 - x0, y1 - y0))
        # Frames line up by the body: what survives a 3 x 3 opening, which arms, tools and tears do not.
        core = [(x, y) for (x, y) in pts if all((x + dx, y + dy) in pts for dx in (-1, 0, 1) for dy in (-1, 0, 1))]
        core = core or list(pts)
        # Truncated rather than rounded: round() takes halves to the even side, which moves every other frame.
        cx = int(sum(x for x, _ in core) / len(core))
        cy = int(sum(y for _, y in core) / len(core))
        frames.append({(x - cx, y - cy): palette[v] for (x, y), v in pts.items()})
    pack(frames, art / f"{name}.png")


# Clawd as Claude Code 2.1.308 draws it on its welcome screen: two rows of quadrant blocks over a row of feet,
# the eyes being the quadrants left in the background colour. A quadrant of a terminal cell is twice as tall
# as it is wide, so here it is one pixel wide and two tall.
ARMS = {  # the left and right ends of the first row, then of the second
    "down": (" ▐", "", "▝▜", "█▀"),
    "up": ("▗▟", "▄", " ▜", "█▘"),
    "right-up": (" ▐", "▄", "▝▜", "█▘"),  # Claude Code's "one-up"
    "left-up": ("▗▟", "", " ▜", "█▀"),  # the same with the other arm
}
EYES = {  # spans of glyphs from the third column on; a lid is a bar in the background colour on the body
    "open": [("▛███▛█", False)],
    "left": [("▟███▟█", False)],
    "right": [("█▟███▟", False)],
    "closed": [("▂", True), ("███", False), ("▂", True), ("█", False)],
    "wink": [("▛███", False), ("▂", True), ("█", False)],
}
FEET = " ▝▝   ▝▝ "
QUADRANTS = {" ": "", "▘": "a", "▝": "b", "▖": "c", "▗": "d", "▌": "ac", "▐": "bd", "▀": "ab", "▄": "cd",
             "▛": "abc", "▜": "abd", "▙": "acd", "▟": "bcd", "█": "abcd"}
BODY, BACK = (0xD7, 0x77, 0x57), (0x00, 0x00, 0x00)  # clawd_body and clawd_background


def clawd(arms="down", eyes="open", at=(0, 0)):
    r1l, r1r, r2l, r2r = ARMS[arms]
    spans, col = [(0, 0, r1l, None, False)], 2
    for glyphs, lid in EYES[eyes]:
        spans.append((col, 0, glyphs, BACK, lid))
        col += len(glyphs)
    spans += [(8, 0, r1r, None, False), (0, 1, r2l, None, False), (2, 1, "█████", BACK, False),
              (7, 1, r2r, None, False), (0, 2, FEET, None, False)]
    out = {}
    for col, row, glyphs, back, lid in spans:
        for i, glyph in enumerate(glyphs):
            x, y = at[0] + 2 * (col + i), at[1] + 4 * row
            if lid:
                out.update({(x + q % 2, y + q // 2): BODY for q in range(6)})
                out.update({(x, y + 3): BACK, (x + 1, y + 3): BACK})
                continue
            for q, (dx, dy) in zip("abcd", ((0, 0), (1, 0), (0, 2), (1, 2))):
                colour = BODY if q in QUADRANTS[glyph] else back
                if colour:
                    out[(x + dx, y + dy)] = out[(x + dx, y + dy + 1)] = colour
    return out


INK = {"g": "#808080", "G": "#C2C3C7", "d": "#5F574F", "w": "#AB5236", "s": "#6E3A1C", "v": "#00E436",
       "y": "#FFEC27", "t": "#88E4F8", "p": "#FF77A8", "r": "#FF004D"}


def stamp(art, x0, y0):
    """A small drawing in the letters of INK, any other letter clear, its top left at (x0, y0)."""
    return {(x0 + x, y0 + y): rgb(INK[ch]) for y, line in enumerate(art.split()) for x, ch in enumerate(line) if ch in INK}


def scene(arms, eyes, *props, at=(0, 0)):
    """Clawd with props in front of it, in the order given."""
    out = clawd(arms, eyes, at)
    for prop in props:
        out.update(prop)
    return out


LAPTOP = stamp("""
    ggggggggggg
    ggggggggggg
    gggggGggggg
    ggggggggggg
  .ddddddddddddd
""", 3, 6)
SWEAT = stamp(".t tt", 0, -1)
SOIL = stamp("""
    ssssssssssssssssssssss
    swsssssswssssssswsssss
""", 0, 10)
SPROUT = stamp("v.v .v. .v.", 0, 7)
QUESTION = ".yy. y..y ...y ..y. .... ..y."
SPARKLE = ".y. yyy .y."
HEART = ".r.r. rrrrr .rrr. ..r.."


def claude_frames():
    return [
        scene("left-up", "open", LAPTOP),
        scene("right-up", "open", LAPTOP, SWEAT),
        scene("right-up", "right", SOIL, SPROUT, stamp("wGG w.G w.. w.. w.. w.. w.. w.. w..", 18, -3)),
        scene("down", "closed", SOIL, SPROUT, SWEAT,
              stamp("w... .w.. ..w. ...w ..GG", 18, 5), stamp("..s. .... ...s .... ....", 19, 3)),
        scene("right-up", "right", stamp(QUESTION, 16, -5)),
        scene("right-up", "open", stamp(QUESTION, 16, -6)),
        scene("up", "closed", stamp(SPARKLE, -2, -4), stamp(SPARKLE, 17, -3), stamp("y", 19, 4), stamp("y", -1, 3), at=(0, -2)),
        scene("down", "wink", stamp("pp", 4, 4), stamp("pp", 12, 4), stamp(HEART, 15, -4)),
        # Tears spurt from the outer corner of each shut eye, arc out and rain down.
        scene("down", "closed", stamp("..tt.. .t..t. t....t t..... t..... t..... ...... t.....", -2, 1),
              stamp("..tt.. .t..t. t....t .....t .....t .....t ...... .....t", 14, 1)),
        scene("down", "open", stamp("t t t t", 5, 4), stamp("t t t t", 13, 4), stamp("ttttttttttttttttt", 1, 10)),
    ]


def claude(art=ART):
    pack(claude_frames(), art / "claude.png")


def check():
    # The glyph tables give Clawd as Claude Code shows it, row for row.
    standing = clawd()
    drawn = ["".join("#" if standing.get((x, y)) == BODY else "K" if (x, y) in standing else "." for x in range(18))
             for y in range(10)]
    assert drawn == [
        "...#############..", "...#############..", "...##K#######K##..", "...##K#######K##..",
        ".#################", ".#################", "...#############..", "...#############..",
        "...#.#.......#.#..", "...#.#.......#.#..",
    ], "\n".join(drawn)
    assert len(claude_frames()) == FRAMES
    with tempfile.TemporaryDirectory() as d:
        d = pathlib.Path(d)
        # A sheet of four by three cells, each holding a checkerboard of 10 x 8 art pixels three image pixels big
        # in black and a colour of its own, a little off the middle of its cell; cut has to find the grid and
        # line them up.
        unit, cell, black = 3, 24, rgb(BASE[0])
        sheet = [[(255, 0, 255)] * (4 * cell * unit) for _ in range(3 * cell * unit)]
        colours = [rgb(c) for c in BRAND["pi"] + BASE[8:12] + BASE[1:6]]
        for k in range(12):
            left = (k % 4) * cell + 6 + k % 3
            top = (k // 4) * cell + 8 + k % 2
            for y in range(8):
                for x in range(10):
                    for v in range(unit):
                        ink = colours[k] if (x + y) % 2 else black
                        sheet[(top + y) * unit + v][(left + x) * unit:(left + x + 1) * unit] = [ink] * unit
        write_rgba(d / "sheet.png", sheet)
        cut(d / "sheet.png", "pi", art=d)
        w, h, px = mascots.read_png(d / "pi.png")
        assert (w, h) == (FRAMES * 10, 10), (w, h)
        for k in range(FRAMES):
            ink = {(x - k * h, y) for y in range(h) for x in range(k * h, (k + 1) * h) if px[y][x][3]}
            assert len(ink) == 80, f"frame {k}: {len(ink)} pixels"
            assert {px[y][x][:3] for x, y in ((x + k * h, y) for x, y in ink)} == {colours[k], black}, f"frame {k}"
            # Every frame sits in the same place of its cell.
            assert min(x for x, _ in ink) == 0 and min(y for _, y in ink) == 1, f"frame {k}: {sorted(ink)[:3]}"
        claude(art=d)
        w, h, _ = mascots.read_png(d / "claude.png")
        assert w == FRAMES * h, (w, h)
    print("check passed")


if __name__ == "__main__":
    if sys.argv[1:2] == ["cut"]:
        cut(sys.argv[2], sys.argv[3])
    elif sys.argv[1:2] == ["claude"]:
        claude()
    elif sys.argv[1:2] == ["check"]:
        check()
    else:
        sys.exit(__doc__)
