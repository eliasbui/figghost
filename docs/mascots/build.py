#!/usr/bin/env python3
"""Mascot scenes for figghost: pixel sprites drawn by Codex, animated here as SVG.

  build.py extract <sheet.png> <name>   cut a generated 4 x 3 sprite sheet into ART/<name>.png
  build.py compose <sheet.png> <name>   cut a generated sheet of loose sprites into ART/<name>.png
  build.py [name ...]                   write <name>-<scene>.svg for every art/*.png (or the names given)
  build.py check                        self-test, then check every SVG here

ART/<name>.png holds 12 frames of CELL x CELL art pixels in one row (1 image pixel = 1 art
pixel), so it can be touched up in any pixel editor. Frame order is FRAMES below. The app draws
the same files (crates/app-ui/src/features/mascot), so they live with it.
"""
import math, pathlib, re, struct, subprocess, sys, tempfile, zlib

HERE = pathlib.Path(__file__).resolve().parent
ART = HERE.parent.parent / "crates" / "app-ui" / "assets" / "mascots"
CELL = 48
FRAMES = ["idle", "blink", "look-left", "look-right", "alert", "pull-a", "pull-b",
          "happy", "proud", "sad", "wave-a", "wave-b"]
PALETTE = ["#000000", "#1D2B53", "#7E2553", "#008751", "#AB5236", "#5F574F", "#C2C3C7", "#FFF1E8",
           "#FF004D", "#FFA300", "#FFEC27", "#00E436", "#29ADFF", "#83769C", "#FF77A8", "#FFCCAA"]
RGB = [tuple(int(c[i:i + 2], 16) for i in (1, 3, 5)) for c in PALETTE]
# Palette swaps applied when a sheet is cut. The spider's body comes out navy, the colour of the
# demo desktop, so it would look hollow without one.
RECOLOR = {"spider": {1: 2}}


# PNG in and out, 8-bit RGB or RGBA, no interlace: what image_gen writes and what we write.

def read_png(path):
    data = pathlib.Path(path).read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        # Image generators sometimes hand back JPEG under a .png name; let macOS convert it.
        with tempfile.TemporaryDirectory() as d:
            subprocess.run(["sips", "-s", "format", "png", str(path), "--out", f"{d}/x.png"], check=True, capture_output=True)
            data = pathlib.Path(f"{d}/x.png").read_bytes()
    pos, idat = 8, b""
    while pos < len(data):
        n = int.from_bytes(data[pos:pos + 4], "big")
        kind, body = data[pos + 4:pos + 8], data[pos + 8:pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
    if depth != 8 or interlace or ctype not in (2, 6):
        raise ValueError(f"{path}: need 8-bit RGB or RGBA without interlace")
    bpp = 4 if ctype == 6 else 3
    raw, stride = zlib.decompress(idat), w * bpp
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(h):
        f, line = raw[i], bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        if f == 1:
            for x in range(bpp, stride):
                line[x] = (line[x] + line[x - bpp]) & 255
        elif f == 2:
            line = bytearray((a + b) & 255 for a, b in zip(line, prev))
        elif f == 3:
            for x in range(stride):
                line[x] = (line[x] + ((line[x - bpp] if x >= bpp else 0) + prev[x]) // 2) & 255
        elif f == 4:
            for x in range(stride):
                a = line[x - bpp] if x >= bpp else 0
                b, c = prev[x], prev[x - bpp] if x >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, [[tuple(r[x * bpp:x * bpp + bpp]) + ((255,) if bpp == 3 else ()) for x in range(w)] for r in rows]


def write_png(path, grid):
    """grid: rows of palette indices, -1 for transparent."""
    raw = b""
    for row in grid:
        raw += b"\x00" + b"".join(bytes(RGB[v] + (255,)) if v >= 0 else b"\x00\x00\x00\x00" for v in row)
    chunk = lambda k, d: struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d))
    pathlib.Path(path).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(
        ">IIBBBBB", len(grid[0]), len(grid), 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def classify(px):
    """Palette index of a pixel, or -1 for the magenta key or transparency."""
    r, g, b, a = px
    if a < 128 or (r > 160 and b > 160 and g < 110):
        return -1
    return min(range(16), key=lambda i: (RGB[i][0] - r) ** 2 + (RGB[i][1] - g) ** 2 + (RGB[i][2] - b) ** 2)


def block_grid(positions, cell):
    """Art pixel size and phase from the image positions where the colour changes. cell is the
    width of one frame in image pixels; a frame is 16 to 64 art pixels across."""
    positions = positions[::max(1, len(positions) // 6000)]

    def strength(u):
        c = sum(math.cos(2 * math.pi * p / u) for p in positions)
        s = sum(math.sin(2 * math.pi * p / u) for p in positions)
        return math.hypot(c, s) / len(positions), math.atan2(s, c)

    coarse = [(strength(u / 10)[0], u / 10) for u in range(int(cell / 64 * 10), int(cell / 16 * 10) + 1)]
    best = max(r for r, _ in coarse)
    # Halves and thirds of the true size fit every edge too; the largest size that fits well wins.
    u0 = max(u for r, u in coarse if r >= 0.6 * best)
    u = max((u0 + d / 200 for d in range(-20, 21)), key=lambda v: strength(v)[0])
    return u, (strength(u)[1] / (2 * math.pi) * u) % u


def clean(pts, size):
    """Drop what a generator adds around a sprite: faint cell borders and lone specks."""
    seen, parts = set(), []
    for p in pts:
        if p in seen:
            continue
        part, todo = [], [p]
        seen.add(p)
        while todo:
            x, y = todo.pop()
            part.append((x, y))
            for q in ((x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1)):
                if q in pts and q not in seen:
                    seen.add(q)
                    todo.append(q)
        parts.append(part)
    keep, biggest = {}, max((len(p) for p in parts), default=0)
    for part in parts:
        xs, ys = [x for x, _ in part], [y for _, y in part]
        thin, long = min(max(xs) - min(xs), max(ys) - min(ys)) < 2, max(max(xs) - min(xs), max(ys) - min(ys)) >= size // 2
        # A neighbouring frame's edge cut off by the split touches the border and is small.
        stray = (min(xs) == 0 or min(ys) == 0 or max(xs) >= size - 1 or max(ys) >= size - 1) and len(part) < biggest // 6
        if (thin and long) or stray or len(part) <= 2:
            continue
        keep.update({p: pts[p] for p in part})
    return keep


def extract(sheet, name):
    w, h, px = read_png(sheet)
    lab = [[classify(p) for p in row] for row in px]
    xs = [x for row in lab for x in range(1, w) if row[x] != row[x - 1]]
    ys = [y for x in range(w) for y in range(1, h) if lab[y][x] != lab[y - 1][x]]
    ux, ox = block_grid(xs, w / 4)
    uy, oy = block_grid(ys, h / 3)
    print(f"art pixel {ux:.2f} x {uy:.2f} image pixels")

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
    for k in range(12):
        c, r = k % 4, k // 4
        x0, x1, y0, y1 = gw * c // 4, gw * (c + 1) // 4, gh * r // 3, gh * (r + 1) // 3
        pts = clean({(x - x0, y - y0): grid[y][x] for y in range(y0, y1) for x in range(x0, x1) if grid[y][x] >= 0},
                    min(x1 - x0, y1 - y0))
        # The body survives a 3 x 3 opening; limbs, staffs and tears do not, so frames line up by the body.
        core = [(x, y) for (x, y) in pts if all((x + dx, y + dy) in pts for dx in (-1, 0, 1) for dy in (-1, 0, 1))]
        core = core or list(pts)
        cx = round(sum(x for x, _ in core) / len(core) + 0.5)
        cy = round(sum(y for _, y in core) / len(core) + 0.5)
        swap = RECOLOR.get(name, {})
        frames.append({(x - cx + CELL // 2, y - cy + CELL // 2): swap.get(v, v) for (x, y), v in pts.items()})
    lost = sum(1 for f in frames for x, y in f if not (0 <= x < CELL and 0 <= y < CELL))
    if lost:
        print(f"warning: {lost} pixels fall outside the {CELL} px cells and are dropped")
    out = [[-1] * (CELL * 12) for _ in range(CELL)]
    for k, f in enumerate(frames):
        for (x, y), v in f.items():
            if 0 <= x < CELL and 0 <= y < CELL:
                out[y][k * CELL + x] = v
    ART.mkdir(exist_ok=True)
    write_png(ART / f"{name}.png", out)
    print(f"wrote {ART / (name + '.png')}")


def components(lab):
    """Every group of touching non-key pixels in a label grid, as sets of points."""
    h, w = len(lab), len(lab[0])
    seen, parts = set(), []
    for y in range(h):
        for x in range(w):
            if lab[y][x] < 0 or (x, y) in seen:
                continue
            seen.add((x, y))
            part, todo = set(), [(x, y)]
            while todo:
                qx, qy = todo.pop()
                part.add((qx, qy))
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        q = (qx + dx, qy + dy)
                        if 0 <= q[0] < w and 0 <= q[1] < h and q not in seen and lab[q[1]][q[0]] >= 0:
                            seen.add(q)
                            todo.append(q)
            parts.append(part)
    return parts


def box(part):
    xs, ys = [x for x, _ in part], [y for _, y in part]
    return min(xs), max(xs), min(ys), max(ys)


def box_gap(a, b):
    """How far apart two boxes are, 0 or less when they touch."""
    ax0, ax1, ay0, ay1 = box(a)
    bx0, bx1, by0, by1 = box(b)
    return max(bx0 - ax1, ax0 - bx1, by0 - ay1, ay0 - by1)


def phase(positions, u):
    """Where the grid of art pixels starts in one sprite: the average position of its edges, modulo u."""
    c = sum(math.cos(2 * math.pi * p / u) for p in positions)
    s = sum(math.sin(2 * math.pi * p / u) for p in positions)
    return (math.atan2(s, c) / (2 * math.pi) * u) % u


def fit_scale(edges, u0):
    """The art pixel size that puts the most edges of every sprite on the grid lines. `block_grid` is
    a few per cent out, and a few per cent is a whole art pixel over the 16 to 64 art pixels a sprite is
    across, which shows up as a chewed edge. Halves and thirds of the true size fit the edges of one
    sprite just as well, so all the sprites are fitted together, which is what rules them out."""
    def cost(u):
        total = 0.0
        for positions in edges:
            ox = phase(positions, u)
            total += sum((min((p - ox) % u, u - (p - ox) % u) / u) ** 2 for p in positions) / len(positions)
        return total / len(edges)

    sweep = [u0 * (1 + d / 200) for d in range(-40, 41)]
    return min(sweep, key=lambda u: (round(cost(u), 6), abs(u - u0)))


def compose(sheet, name, art=ART):
    """Cut a generated sheet of loose sprites into the twelve frames of ART/<name>.png: the same job
    as `extract`, for a sheet whose sprites answer to no grid. The sprites are found by what touches
    what, put in reading order, sampled on the art's own pixel grid (the size of the most common
    one, so no frame is drawn at another scale) and stood in the frame with their middle over the
    middle, which is how `extract` lines up the frames of a grid sheet."""
    w, h, px = read_png(sheet)
    lab = [[classify(p) for p in row] for row in px]
    parts = [p for p in components(lab) if len(p) > (w * h) // 2000]
    if not parts:
        raise ValueError(f"{sheet}: no sprite found")
    while True:  # a rope or a thread a sprite holds onto is part of that sprite, not one of its own
        near = next(((i, j) for i in range(len(parts)) for j in range(i + 1, len(parts))
                     if box_gap(parts[i], parts[j]) <= 3), None)
        if not near:
            break
        i, j = near
        parts[i] |= parts.pop(j)
    biggest = max(len(p) for p in parts)
    parts = [p for p in parts if len(p) > biggest // 5]
    if not all(box(p)[0] > 0 and box(p)[1] < w - 1 and box(p)[2] > 0 and box(p)[3] < h - 1 for p in parts):
        print("warning: a sprite touches the edge of the sheet, so part of it may be missing")

    def edges(part):
        """Where the ink of one sprite starts, stops or changes colour: the art's pixel grid shows
        up in those positions, and only in that sprite's own, so one sprite cannot set another's. Both
        sides of the sprite are read, one art pixel past its box, or its own outline is unseen."""
        x0, x1, y0, y1 = box(part)
        xs, ys = [], []
        for y in range(y0, min(y1 + 2, h)):
            for x in range(x0, min(x1 + 2, w)):
                here, before, above = (x, y) in part, (x - 1, y) in part, (x, y - 1) in part
                if here != before or (here and before and lab[y][x] != lab[y][x - 1]):
                    xs.append(x)
                if here != above or (here and above and lab[y][x] != lab[y - 1][x]):
                    ys.append(y)
        return xs, ys

    heights = sorted(box(p)[3] - box(p)[2] for p in parts)
    row_gap = heights[len(heights) // 2] * 0.6
    rows = []
    for p in sorted(parts, key=lambda p: (box(p)[2] + box(p)[3]) / 2):
        cy = (box(p)[2] + box(p)[3]) / 2
        cx = (box(p)[0] + box(p)[1]) / 2
        if not rows or cy - (sum(y for y, _, _ in rows[-1]) / len(rows[-1])) > row_gap:
            rows.append([])
        rows[-1].append((cy, cx, p))
    parts = [p for row in rows for _, _, p in sorted(row, key=lambda item: item[1])]
    if len(parts) != 12:
        raise ValueError(f"{sheet}: {len(parts)} sprites in {len(rows)} rows, need 12; "
                         f"sizes {sorted(len(p) for p in parts)}")

    part_edges = [edges(p) for p in parts]
    rough_x = [block_grid(xs, box(p)[1] - box(p)[0] + 1)[0] for (xs, _), p in zip(part_edges, parts)]
    rough_y = [block_grid(ys, box(p)[3] - box(p)[2] + 1)[0] for (_, ys), p in zip(part_edges, parts)]
    ux0 = sorted(rough_x)[len(rough_x) // 2]
    uy0 = sorted(rough_y)[len(rough_y) // 2]
    ux = fit_scale([xs for xs, _ in part_edges], ux0)
    uy = fit_scale([ys for _, ys in part_edges], uy0)
    print(f"one art pixel is {ux:.2f} x {uy:.2f} image pixels")

    cells = []
    for frame, (part, (xs, ys)) in enumerate(zip(parts, part_edges)):
        x0, x1, y0, y1 = box(part)
        ox, oy = phase(xs, ux), phase(ys, uy)

        def sample(cx, cy):
            votes = {}
            for y in range(int(cy - uy * 0.2), int(cy + uy * 0.2) + 1):
                for x in range(int(cx - ux * 0.2), int(cx + ux * 0.2) + 1):
                    if (x, y) in part:
                        votes[lab[y][x]] = votes.get(lab[y][x], 0) + 1
            return max(votes, key=votes.get) if votes else -1

        grid = {}
        for i in range(math.floor((x0 - ox) / ux), math.ceil((x1 - ox) / ux) + 1):
            for j in range(math.floor((y0 - oy) / uy), math.ceil((y1 - oy) / uy) + 1):
                v = sample(ox + (i + 0.5) * ux, oy + (j + 0.5) * uy)
                if v >= 0:
                    grid[(i, j)] = v
        if not grid:
            raise ValueError(f"{sheet}: frame {frame} ({FRAMES[frame]}) came out empty")
        # The middle of the body, which is where the frame holds the sprite, as in `extract`: what
        # survives a 3 x 3 opening is the body, and arms, staffs and tears move around it.
        core = [(x, y) for (x, y) in part if all((x + dx, y + dy) in part for dx in (-1, 0, 1) for dy in (-1, 0, 1))] or list(part)
        gi = math.floor((sum(x for x, _ in core) / len(core) - ox) / ux)
        gj = math.floor((sum(y for _, y in core) / len(core) - oy) / uy)
        swap = RECOLOR.get(name, {})
        cells.append({(i - gi + CELL // 2, j - gj + CELL // 2): swap.get(v, v) for (i, j), v in grid.items()})
        gw = max(i for i, _ in grid) - min(i for i, _ in grid) + 1
        gh = max(j for _, j in grid) - min(j for _, j in grid) + 1
        print(f"  {FRAMES[frame]:10s} {gw:2d} x {gh:2d} art pixels")

    lost = sum(1 for c in cells for x, y in c if not (0 <= x < CELL and 0 <= y < CELL))
    if lost:
        print(f"warning: {lost} pixels fall outside the {CELL} px cells and are dropped")
    out = [[-1] * (CELL * 12) for _ in range(CELL)]
    for k, cell in enumerate(cells):
        for (x, y), v in cell.items():
            if 0 <= x < CELL and 0 <= y < CELL:
                out[y][k * CELL + x] = v
    art.mkdir(exist_ok=True)
    write_png(art / f"{name}.png", out)
    print(f"wrote {art / (name + '.png')}")


# Sprites

class Sprite:
    def __init__(self, name):
        w, h, px = read_png(ART / f"{name}.png")
        self.name = name
        self.frames = {}
        for k, fname in enumerate(FRAMES):
            pts = {(x, y): classify(px[y][k * CELL + x]) for y in range(CELL) for x in range(CELL)}
            self.frames[fname] = {p: v for p, v in pts.items() if v >= 0}
        idle, blink = self.frames["idle"], self.frames["blink"]
        # Sprites drawn on a finer grid come out bigger; 0.75 keeps them near the others and is
        # still a whole 3 screen pixels per art pixel at the 960 x 600 size.
        height = max(y for _, y in idle) - min(y for _, y in idle) + 1
        self.k = 1 if height <= 26 else .75
        mid = [y for (x, y) in idle if abs(x - CELL // 2) <= 1]
        self.top = (min(mid) - CELL // 2) * self.k  # top of the body above its centre, where a thread attaches
        self.bottom = (max(mid) - CELL // 2 + 1) * self.k
        # The eyes are what changes between idle and blink; a tear starts under the left one.
        eyes = [p for p in set(idle) | set(blink) if idle.get(p) != blink.get(p) and p[0] < CELL // 2] or [(CELL // 2 - 3, CELL // 2)]
        self.eye = ((min(x for x, _ in eyes) - CELL // 2) * self.k, (max(y for _, y in eyes) + 1 - CELL // 2) * self.k)

    def defs(self, used):
        out = []
        for fname, pts in self.frames.items():
            if fname not in used:
                continue
            by_colour = {}
            for y in range(CELL):
                x = 0
                while x < CELL:
                    v = pts.get((x, y), -1)
                    if v < 0:
                        x += 1
                        continue
                    n = 1
                    while pts.get((x + n, y), -1) == v:
                        n += 1
                    by_colour.setdefault(v, []).append(f"M{x} {y}h{n}v1h-{n}z")
                    x += n
            paths = "".join(f'<path fill="{PALETTE[v]}" d="{"".join(d)}"/>' for v, d in sorted(by_colour.items()))
            out.append(f'<g id="{self.name}-{fname}">{paths}</g>')
        return "\n".join(out)


# Timelines. A track is a list of keys (percent, values, easing to the next key). The same
# track writes the CSS keyframes and, evaluated at the scene's still moment, the attributes
# that reduced motion shows.

STEP = "step-end"
EASE_OUT = "cubic-bezier(.2,.8,.3,1)"
EASE_IN_OUT = "cubic-bezier(.45,0,.55,1)"
EASE_IN = "cubic-bezier(.5,0,.9,.6)"
SPRING = "cubic-bezier(.3,1.6,.5,1)"


class Track:
    def __init__(self, keys, default="linear"):
        self.keys = []
        for k in sorted(keys, key=lambda k: k[0]):
            pct, vals = k[0], dict(k[1])
            ease = k[2] if len(k) > 2 else default
            if self.keys:
                vals = {**self.keys[-1][1], **vals}
            self.keys.append((pct, vals, ease))
        first = self.keys[0][1]
        self.keys = [(p, {**first, **v}, e) for p, v, e in self.keys]
        if self.keys[0][0] > 0:
            self.keys.insert(0, (0, self.keys[0][1], self.keys[0][2]))
        if self.keys[-1][0] < 100:
            self.keys.append((100, self.keys[-1][1], self.keys[-1][2]))

    def at(self, pct):
        for (p0, v0, e), (p1, v1, _) in zip(self.keys, self.keys[1:]):
            if p0 <= pct < p1:
                if e == STEP:
                    return v0
                t = (pct - p0) / (p1 - p0)
                return {k: v0[k] + (v1[k] - v0[k]) * t for k in v0}
        return self.keys[-1][1]


def fmt(n):
    return f"{n:.2f}".rstrip("0").rstrip(".") if n != int(n) else str(int(n))


def css_of(v):
    parts = []
    if "x" in v or "y" in v:
        parts.append(f"translate({fmt(v.get('x', 0))}px,{fmt(v.get('y', 0))}px)")
    if "r" in v:
        parts.append(f"rotate({fmt(v['r'])}deg)")
    if "sx" in v or "sy" in v or "s" in v:
        parts.append(f"scale({fmt(v.get('sx', v.get('s', 1)))},{fmt(v.get('sy', v.get('s', 1)))})")
    out = []
    if parts:
        out.append("transform:" + " ".join(parts))
    if "o" in v:
        out.append(f"opacity:{fmt(v['o'])}")
    return ";".join(out)


def attrs_of(v):
    parts = []
    if "x" in v or "y" in v:
        parts.append(f"translate({fmt(v.get('x', 0))} {fmt(v.get('y', 0))})")
    if "r" in v:
        parts.append(f"rotate({fmt(v['r'])})")
    if "sx" in v or "sy" in v or "s" in v:
        parts.append(f"scale({fmt(v.get('sx', v.get('s', 1)))} {fmt(v.get('sy', v.get('s', 1)))})")
    out = f' transform="{" ".join(parts)}"' if parts else ""
    if "o" in v:
        out += f' opacity="{fmt(v["o"])}"'
    return out


class Scene:
    def __init__(self, sprite, title, desc, seconds, still):
        self.sprite, self.title, self.desc, self.seconds, self.still = sprite, title, desc, seconds, still
        self.rules, self.n, self.defs, self.used = [], 0, [], set()

    def anim(self, track, inner, prefix):
        """An animated <g> around inner markup, driven by track."""
        self.n += 1
        name = f"{prefix}{self.n}"
        frames = []
        for pct, vals, ease in track.keys:
            frames.append(f"{fmt(pct)}%{{{css_of(vals)};animation-timing-function:{ease}}}")
        self.rules.append(f".{name}{{animation-name:{name}}}@keyframes {name}{{{''.join(frames)}}}")
        return f'<g class="a {name}"{attrs_of(track.at(self.still))}>{inner}</g>'

    def svg(self, stage, payload, actor, pointer):
        return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 150" width="960" height="600" shape-rendering="crispEdges" role="img" aria-labelledby="title desc">
<title id="title">{self.title}</title>
<desc id="desc">{self.desc}</desc>
<style>
.a{{animation-duration:{self.seconds}s;animation-iteration-count:infinite;animation-delay:0s}}
text{{font-family:Menlo,Monaco,'Courier New',monospace}}
{chr(10).join(self.rules)}
@media (prefers-reduced-motion:reduce){{.a{{animation:none!important}}}}
</style>
<defs>
{self.sprite.defs(self.used)}
{POINTER_DEF}
{''.join(self.defs)}
</defs>
<g id="stage">{stage}</g>
<g id="payload">{payload}</g>
<g id="actor">{actor}</g>
<g id="pointer">{pointer}</g>
</svg>
"""


# Shared stage pieces

NOTCH_Y, PANEL_Y = 10, 44  # bottom edge of the closed notch and of the open Hub panel
HOME, OPEN_HOME = (120, 30), (120, 62)  # the actor's body centre under the closed notch and the open panel
TILES = [86, 110, 134]  # left edges of the Hub tiles, 20 x 20 at y 14
SILK = "#83769C"  # reads on the navy desktop, the cream windows and the black notch
POINTER_DEF = ('<g id="arrow"><path fill="#FFF1E8" d="M-1 -1h3v1h1v1h1v1h1v1h1v1h1v1h1v2h-3v1h1v2h1v2h-3v-1h-1v-2h-1v1h-1v1h-2z"/>'
               '<path fill="#000000" d="M0 0h1v1h1v1h1v1h1v1h1v1h1v1h-3v2h1v2h-1v-1h-1v-2h-1v1h-1v1h-1z"/></g>')


def notch_path(x0, x1, y1, r):
    """Black shape hanging from the top edge with a staircase on its lower corners."""
    d = f"M{x0} 0H{x1}V{y1 - r}"
    for i in range(r):
        d += f"H{x1 - i - 1}V{y1 - r + i + 1}"
    d += f"H{x0 + r}"
    for i in range(r):
        d += f"V{y1 - i - 1}H{x0 + r - i - 1}"
    return f'<path fill="#000000" d="{d}Z"/>'


def window():
    return ('<rect x="26" y="62" width="192" height="84" fill="#000000" opacity=".35"/>'
            '<rect x="24" y="60" width="192" height="84" fill="#5F574F"/>'
            '<rect x="25" y="61" width="190" height="82" fill="#FFF1E8"/>'
            '<rect x="25" y="61" width="190" height="8" fill="#C2C3C7"/>'
            '<rect x="29" y="64" width="2" height="2" fill="#FF004D"/>'
            '<rect x="34" y="64" width="2" height="2" fill="#FFA300"/>'
            '<rect x="39" y="64" width="2" height="2" fill="#00E436"/>')


def shown(s, keys, inner, prefix="v"):
    """inner, visible according to (percent, on) pairs; hidden before the first."""
    return s.anim(Track([(0, {"o": 0}, STEP)] + [(p, {"o": 1 if on else 0}, STEP) for p, on in keys]), inner, prefix)


def menu(s, x, y, items, hot, hot_fill, hot_text, show, pick):
    """Context menu with its top left corner at (x, y): opens at show %, the hot row lights at
    pick - 4 %, and it closes at pick + 2 %."""
    wdt = 6 + max(len(t) for t in items) * 3
    rows = "".join(f'<text x="{x + 3}" y="{y + 6 + 7 * i}" font-size="5" fill="#1D2B53">{t}</text>'
                   for i, t in enumerate(items))
    hl = shown(s, [(pick - 4, 1), (pick + 2, 0)],
               f'<rect x="{x + 1}" y="{y + 1 + 7 * hot}" width="{wdt - 2}" height="7" fill="{hot_fill}"/>'
               f'<text x="{x + 3}" y="{y + 6 + 7 * hot}" font-size="5" fill="{hot_text}">{items[hot]}</text>')
    body = (f'<rect x="{x + 1}" y="{y + 1}" width="{wdt}" height="{7 * len(items) + 2}" fill="#000000" opacity=".3"/>'
            f'<rect x="{x}" y="{y}" width="{wdt}" height="{7 * len(items) + 2}" fill="#5F574F"/>'
            f'<rect x="{x + 1}" y="{y + 1}" width="{wdt - 2}" height="{7 * len(items)}" fill="#C2C3C7"/>{rows}{hl}')
    return shown(s, [(show, 1), (pick + 2, 0)], body)


def closed_notch(s, gulp=None):
    shape = notch_path(92, 148, NOTCH_Y, 2)
    if gulp is None:
        return shape
    t = Track([(0, {"x": 0, "sx": 1, "sy": 1}), (gulp, {}, EASE_OUT), (gulp + 2, {"x": -9.6, "sx": 1.08, "sy": 1.3}, SPRING),
               (gulp + 6, {"x": 0, "sx": 1, "sy": 1})])
    return s.anim(t, shape, "n")


def hub_panel(s, keys, contents):
    """The notch growing into the Hub panel. keys: (percent, openness 0 or 1, easing)."""
    sx0, sy0 = 56 / 120, NOTCH_Y / PANEL_Y
    shape = Track([(p, {"x": 120 * (1 - (sx0 + (1 - sx0) * o)), "sx": sx0 + (1 - sx0) * o, "sy": sy0 + (1 - sy0) * o}, e)
                   for p, o, e in keys])
    inside = Track([(p, {"o": o}, e if o else STEP) for p, o, e in keys])
    return (s.anim(shape, notch_path(60, 180, PANEL_Y, 3), "h") +
            s.anim(inside, '<text x="65" y="9" font-size="5" fill="#C2C3C7">Hub</text>' + contents, "h"))


def tile(x, inner=""):
    return f'<rect x="{x}" y="14" width="20" height="20" fill="#1D2B53"/><rect x="{x}.5" y="14.5" width="19" height="19" fill="none" stroke="#5F574F"/>{inner}'


def badge(s, at, x=151):
    t = Track([(0, {"y": 0, "o": 0}, STEP), (at, {"o": 1}), (at + 8, {"y": -3, "o": 1}), (at + 11, {"o": 0})])
    return s.anim(t, f'<rect x="{x}" y="2" width="11" height="7" fill="#00E436"/>'
                     f'<text x="{x + 1}" y="7.5" font-size="5" fill="#000000">+1</text>', "b")


def file_icon(x, y, kind):
    body = (f'<path fill="#FFF1E8" d="M{x} {y}h8l3 3v12h-11z"/><path fill="#5F574F" d="M{x} {y}h8v1h-7v13h10v-11h1v12h-12z"/>'
            f'<path fill="#C2C3C7" d="M{x + 8} {y}h1v2h2v1h-3z"/>')
    band = {"text": f'<rect x="{x + 2}" y="{y + 5}" width="6" height="1" fill="#5F574F"/><rect x="{x + 2}" y="{y + 8}" width="7" height="1" fill="#5F574F"/><rect x="{x + 2}" y="{y + 11}" width="5" height="1" fill="#5F574F"/>',
            "picture": f'<rect x="{x + 2}" y="{y + 5}" width="7" height="6" fill="#29ADFF"/><rect x="{x + 6}" y="{y + 6}" width="2" height="2" fill="#FFEC27"/><path fill="#008751" d="M{x + 2} {y + 9}h2v-1h2v1h1v-1h2v3h-7z"/>',
            "zip": f'<rect x="{x + 2}" y="{y + 4}" width="7" height="2" fill="#FFA300"/><rect x="{x + 5}" y="{y + 7}" width="2" height="6" fill="#5F574F"/><rect x="{x + 5}" y="{y + 8}" width="2" height="1" fill="#C2C3C7"/><rect x="{x + 5}" y="{y + 10}" width="2" height="1" fill="#C2C3C7"/>',
            "pdf": f'<rect x="{x + 2}" y="{y + 5}" width="7" height="3" fill="#FF004D"/><rect x="{x + 2}" y="{y + 10}" width="6" height="1" fill="#5F574F"/>'}[kind]
    return body + band


def landscape(x, y):
    """The 64 x 40 picture copied in copy-image."""
    return (f'<rect x="{x}" y="{y}" width="64" height="40" fill="#29ADFF"/><rect x="{x + 46}" y="{y + 6}" width="7" height="7" fill="#FFEC27"/>'
            f'<path fill="#5F574F" d="M{x} {y + 30}h8v-4h6v-4h6v4h4v8h-24z"/>'
            f'<path fill="#008751" d="M{x + 14} {y + 34}v-6h6v-6h8v-4h6v4h6v6h6v-4h8v4h6v6z"/>'
            f'<rect x="{x}" y="{y + 34}" width="64" height="6" fill="#00E436"/><rect x="{x}.5" y="{y}.5" width="63" height="39" fill="none" stroke="#5F574F"/>')


def pointer(s, keys):
    """The arrow, moved by keys; a press is a 1 px nudge down and right."""
    return s.anim(Track(keys, STEP), '<use href="#arrow"/>', "p")


def clipped(s, below, inner):
    """Payload that disappears behind the edge of the notch or the panel."""
    s.defs.append(f'<clipPath id="below{below}"><rect x="0" y="{below}" width="240" height="{150 - below}"/></clipPath>')
    return f'<g clip-path="url(#below{below})">{inner}</g>'


# Characters. Each one has its own way of reaching things (ray), of bringing a copy home (haul)
# and of protecting a file on its way out (shield); the stage and its timing are shared.

STYLE = {
    "spider": dict(ray="silk", haul="tugs", hang=True, shield="rect", side=(-18, 4), reach=22, verb="shoots silk at"),
    "frog": dict(ray="tongue", haul="yank", shield="rect", side=(-20, 2), reach=20, verb="flicks its tongue at"),
    "ufo": dict(ray="beam", haul="lift", shield="beam", side=(0, -26), reach=30, verb="beams up"),
    "ghost": dict(ray="wisp", haul="float", shield="bubble", side=(-18, 0), reach=20, verb="reaches with a wisp for"),
    "wizard": dict(ray="magic", strike="bolt", haul="float", shield="circle", side=(-20, 0), reach=22, verb="zaps"),
    "mario": dict(ray="bolt", haul="tugs", shield="rect", side=(-18, 2), reach=20, verb="snatches"),
}


def band(points, half, fill, opacity=1):
    """A ribbon along (x, y) points in a unit-length frame, half wide on each side."""
    left = " ".join(f"{fmt(x - half)} {fmt(y)}" for x, y in points)
    right = " ".join(f"{fmt(x + half)} {fmt(y)}" for x, y in reversed(points))
    return f'<path fill="{fill}" fill-opacity="{opacity}" d="M{left} L{right}Z"/>'


def ray_shape(kind, far=10):
    """What a ray looks like from 0 to 1 along +y, before ray() scales it to length. far: half width
    of a beam where it meets its target."""
    if kind == "silk":
        return f'<rect x="-.5" width="1" height="1" fill="{SILK}"/>'
    if kind == "magic":
        return '<rect x="-.5" width="1" height="1" fill="#29ADFF"/>'
    if kind == "tongue":
        return '<rect x="-1.5" width="3" height="1" fill="#FF77A8"/><rect x="-1.5" width="1" height="1" fill="#7E2553"/>'
    if kind == "beam":
        return (f'<path fill="#FFEC27" fill-opacity=".3" d="M-3 0H3L{far} 1H-{far}Z"/>'
                f'<path fill="#FFF1E8" fill-opacity=".25" d="M-1 0H1L{far - 4} 1H-{far - 4}Z"/>')
    if kind == "wisp":
        pts = [(1.5 * math.sin(i * math.pi / 3), i / 12) for i in range(13)]
        return band(pts, 1, "#83769C", .8) + band(pts, .4, "#FFF1E8", .7)
    if kind == "bolt":
        pts = [(0 if i in (0, 8) else 2.5 if i % 2 else -2.5, i / 8) for i in range(9)]
        return band(pts, 1, "#FFA300") + band(pts, .4, "#FFEC27")
    raise ValueError(kind)


TIPS = {"silk": ("#FFF1E8", "M-1 -3h2v2h2v2h-2v2h-2v-2h-2v-2h2z"),
        "tongue": ("#FF004D", "M-1 -2h2v1h1v2h-1v1h-2v-1h-1v-2h1z"),
        "wisp": ("#83769C", "M-1 -3h2v2h2v2h-2v2h-2v-2h-2v-2h2z"),
        "magic": ("#FFEC27", "M-1 -3h2v2h2v2h-2v2h-2v-2h-2v-2h2z"),
        "bolt": ("#FFEC27", "M-1 -4h2v3h3v2h-3v3h-2v-3h-3v-2h3z")}


def tip(s, track, kind):
    """Where a ray sticks: a splat of silk, the tongue's sticky end, a spark."""
    if kind not in TIPS:
        return ""
    colour, d = TIPS[kind]
    return s.anim(track, f'<path fill="{colour}" d="{d}"/>', "s")


# Actor helpers (shared by every character)

def frames_of(s, track, where):
    """One <use> per sprite frame, each shown while the frame track names it; where moves them all.
    Characters that float bob by a pixel now and then."""
    out = []
    for fname in FRAMES:
        keys, prev = [], 0
        for pct, n in track:
            on = 1 if n == fname else 0
            if on != prev or pct == 0:
                keys.append((pct, {"o": on}, STEP))
                prev = on
        if any(k[1]["o"] for k in keys):
            s.used.add(fname)
            out.append(s.anim(Track(keys, STEP), f'<use href="#{s.sprite.name}-{fname}"/>', "f"))
    body = f'<g transform="scale({s.sprite.k}) translate({-CELL // 2} {-CELL // 2})">{"".join(out)}</g>'
    if not STYLE[s.sprite.name].get("hang"):
        body = s.anim(Track([(p, {"y": -1 if (p // 8) % 2 else 0}, STEP) for p in range(0, 100, 8)]), body, "o")
    return s.anim(where, body, "w")


def ray(s, keys, kind, far=10):
    """A straight ray from (x, y) with direction r and length l (see seg)."""
    out, cur = [], {"o": 1}
    for p, v, *e in keys:
        cur = {**cur, **v}
        out.append((p, {"x": cur["x"], "y": cur["y"], "r": cur["r"], "sy": max(cur["l"], 0.01), "o": cur["o"]}, *e))
    return s.anim(Track(out), ray_shape(kind, far), "l")


def seg(a, b):
    """Rotation and length that make a ray go from a to b."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    return {"x": a[0], "y": a[1], "r": math.degrees(math.atan2(-dx, dy)), "l": math.hypot(dx, dy)}


def at(track, pct):
    v = track.at(pct)
    return v["x"], v["y"]


def dragline(s, where, anchor):
    """Thread from the edge above (anchor: percent -> y) down to the spider's body top, always vertical."""
    keys = []
    for p, v, e in where.keys:
        a = anchor(p)
        keys.append((p, {"x": v["x"], "y": a, "r": 0, "l": v["y"] + s.sprite.top - a}, e))
    return ray(s, keys, "silk")


def tear(s, starts):
    """A tear that drops from under the left eye, once for every start %, relative to the actor."""
    ex, ey = s.sprite.eye
    keys = [(0, {"y": 0, "o": 0}, STEP)]
    for p in starts:
        keys += [(p, {"y": 0, "o": 1}, "ease-in"), (p + 5, {"y": 8, "o": 0}, STEP)]
    return s.anim(Track(keys), f'<rect x="{fmt(ex)}" y="{fmt(ey + 1)}" width="2" height="3" fill="#29ADFF"/>', "t")


def follow(track, keys_at, offset=(0, 0), extra=None):
    """Keys that put something at track's position plus offset at each of keys_at (percent, easing)."""
    out = []
    for p, e in keys_at:
        x, y = at(track, p)
        out.append((p, {"x": x + offset[0], "y": y + offset[1], **(extra(p) if extra else {})}, e))
    return out


# Scenes

def copy_scene(sp, kind):
    st = STYLE[sp.name]
    if kind == "text":
        target, w, h = (106, 92.5), 42, 7
        title, desc = f"{sp.name.title()} copies selected text", f"Cmd and C are pressed; the {sp.name} {st['verb']} the selected words and brings a copy up into the notch while the original stays selected."
    else:
        target, w, h = (120, 96), 64, 40
        title, desc = f"{sp.name.title()} copies a picture", f"Copy Image is chosen from the right click menu; the {sp.name} {st['verb']} the picture and brings a copy up into the notch while the original stays in the window."
    s = Scene(sp, title, desc, 6, 50)
    end_scale = .45 if kind == "text" else .3

    stage = ['<rect width="240" height="150" fill="#1D2B53"/>', closed_notch(s, 62), window()]
    if kind == "text":
        for y, wdt in [(80, 122), (86, 143), (98, 136), (104, 110), (110, 150)]:
            stage.append(f'<rect x="43" y="{y}" width="{wdt}" height="2" fill="#5F574F"/>')
        stage.append('<rect x="43" y="92" width="42" height="2" fill="#5F574F"/><rect x="127" y="92" width="40" height="2" fill="#5F574F"/>')
        chip = ('<rect width="42" height="7" fill="#29ADFF"/><rect x="2" y="3" width="14" height="2" fill="#FFF1E8"/>'
                '<rect x="20" y="3" width="18" height="2" fill="#FFF1E8"/>')
        stage.append(f'<g transform="translate(85 89)">{chip}</g>')
        caps = ('<rect x="175" y="128" width="20" height="10" fill="#5F574F"/><rect x="175" y="127" width="20" height="9" fill="#C2C3C7"/>'
                '<text x="179" y="133.5" font-size="5" fill="#1D2B53">Cmd</text>'
                '<rect x="198" y="128" width="11" height="10" fill="#5F574F"/><rect x="198" y="127" width="11" height="9" fill="#C2C3C7"/>'
                '<text x="201.5" y="133.5" font-size="6" fill="#1D2B53">C</text>')
        stage.append(s.anim(Track([(0, {"o": 0, "y": 0}, STEP), (10, {"o": 1}, STEP), (15, {"y": 1}, STEP), (18, {"y": 0}, STEP),
                                   (88, {"o": 0}, STEP)]), caps, "k"))
        arrow = ""
    else:
        stage.append(landscape(88, 76))
        chip = landscape(0, 0)
        stage.append(menu(s, 120, 97, ["Open Image", "Copy Image", "Save Image As..."], 1, "#29ADFF", "#FFF1E8", 11, 20))
        arrow = pointer(s, [(0, {"x": 196, "y": 140, "o": 1}, EASE_IN_OUT), (8, {"x": 120, "y": 97}, STEP),
                            (10, {"x": 121, "y": 98}, STEP), (11, {"x": 120, "y": 97}, EASE_IN_OUT), (14, {"x": 126, "y": 105}, STEP),
                            (19, {"x": 127, "y": 106}, STEP), (20, {"x": 126, "y": 105}, EASE_IN_OUT), (28, {"x": 196, "y": 140}, STEP),
                            (29, {"o": 0}, STEP), (97, {"o": 1}, STEP)])
    stage.append(badge(s, 62))

    # where the actor is: at home, except the saucer, which flies over the target and back
    if st["haul"] == "lift":
        over = (target[0], target[1] - st["reach"] - h / 2)
        where = Track([(0, {"x": HOME[0], "y": HOME[1]}, STEP), (22, {}, EASE_IN_OUT), (28, {"x": over[0], "y": over[1]}, STEP),
                       (56, {}, EASE_IN_OUT), (59, {"x": HOME[0], "y": HOME[1]}, EASE_IN), (61, {"y": NOTCH_Y + 2}, EASE_OUT),
                       (65, {"y": HOME[1]}, STEP), (70, {"y": HOME[1] - 2}, STEP), (72, {"y": HOME[1]}, STEP)])
    else:
        where = Track([(0, {"x": HOME[0], "y": HOME[1]}, STEP), (26, {"y": HOME[1] - 2}, STEP), (30, {"y": HOME[1]}, STEP),
                       (58, {"y": HOME[1] - 2}, STEP), (60, {"y": HOME[1]}, STEP),
                       (70, {"y": HOME[1] - 2}, STEP), (72, {"y": HOME[1]}, STEP), (78, {"y": HOME[1] - 2}, STEP), (80, {"y": HOME[1]}, STEP)])
    me = at(where, 40)
    mouth = (me[0], me[1] + sp.bottom - 2) if st["haul"] == "lift" else me
    hold = (me[0], me[1] + sp.bottom + 4)
    along = lambda f: (target[0] + (hold[0] - target[0]) * f, target[1] + (hold[1] - target[1]) * f)

    # the copy: from the target to the actor, then into the notch
    keys = [(0, {"x": target[0], "y": target[1], "s": 1, "o": 0}, STEP), (42, {"o": 1}, "linear")]
    if st["haul"] == "tugs":
        steps = [(44, .4, EASE_OUT), (48, .4, "linear"), (50, .75, EASE_OUT), (54, .75, "linear"), (56, 1, EASE_OUT)]
    elif st["haul"] == "yank":
        steps = [(44.5, 1, SPRING), (56, 1, "linear")]
    else:
        steps = [(44.5, .18, "linear"), (47, .36, "linear"), (49.5, .54, "linear"), (52, .72, "linear"), (54.5, .9, "linear"), (56, 1, "linear")]
    keys[-1] = (42, keys[-1][1], steps[0][2] if st["haul"] != "float" else "linear")
    for i, (p, f, _) in enumerate(steps):
        x, y = along(f)
        if st["haul"] == "float":
            x += (4 if i % 2 else -4) * (1 - f)  # drifts on a wave that settles as it arrives
        e = steps[i + 1][2] if i + 1 < len(steps) else EASE_IN_OUT
        extra = {"o": .55 if sp.name == "ghost" and i % 2 else 1}
        keys.append((p, {"x": x, "y": y, "s": 1 + (end_scale - 1) * f, **extra}, e))
    if st["haul"] == "lift":
        keys[-1] = (56, {**keys[-1][1], "o": 0}, STEP)  # swallowed by the saucer, which flies it home
    else:
        keys += [(62, {"x": hold[0], "y": NOTCH_Y - 8, "s": end_scale * .7, "o": 1}, STEP), (63, {"o": 0}, STEP)]
    copy = Track(keys)
    payload = clipped(s, NOTCH_Y, s.anim(copy, f'<g transform="translate({-w / 2} {-h / 2})">{chip}</g>', "c"))

    # the ray: out at 30 to 35 (a flick for the tongue), attached to the copy until it arrives
    strike = st.get("strike", st["ray"])
    out_by = 31.5 if st["ray"] == "tongue" else 34
    hauling = [p for p, _ in [(k[0], 0) for k in keys] if 42 <= p <= 56]
    rays = []
    if strike != st["ray"]:  # a bolt strikes first, then a thinner tether takes over
        rays.append(ray(s, [(0, {**seg(mouth, target), "o": 0}, STEP), (30, {"o": 1}, STEP), (31.5, {"o": .4}, STEP),
                            (32.5, {"o": 1}, STEP), (35, {"o": 0}, STEP)], strike))
    rkeys = [(0, {**seg(mouth, target), "l": 0, "o": 0}, STEP), (30 if strike == st["ray"] else 34, {"o": 1}, EASE_OUT),
             (out_by if strike == st["ray"] else 35, seg(mouth, target), STEP)]
    for i, p in enumerate(hauling):
        nxt = copy.keys[[k[0] for k in copy.keys].index(p)][2]
        rkeys.append((p, seg(mouth, at(copy, p)), nxt))
    rkeys.append((56, {"o": 0}, STEP))
    rays.append(ray(s, rkeys, st["ray"], w / 2 + 3))
    stick = Track([(0, {"x": target[0], "y": target[1], "o": 0}, STEP), (out_by, {"o": 1}, STEP)] +
                  [(p, {"x": at(copy, p)[0], "y": at(copy, p)[1]}, copy.keys[[k[0] for k in copy.keys].index(p)][2]) for p in hauling] +
                  [(45 if st["haul"] == "yank" else 56, {"o": 0}, STEP)])
    effort = ["pull-a", "pull-b"]
    frames = [(0, "idle"), (4, "blink"), (5.5, "idle"), (20, "look-left" if target[0] < HOME[0] - 4 else "alert"), (24, "alert")]
    frames += [(p, effort[i % 2]) for i, p in enumerate(hauling[:-1])] + [(56, "alert"), (64, "happy"), (88, "idle"), (95, "blink"), (96.5, "idle")]
    actor = ([dragline(s, where, lambda p: NOTCH_Y)] if st.get("hang") else []) + rays + [tip(s, stick, strike), frames_of(s, frames, where)]
    return s.svg("".join(stage), payload, "".join(actor), arrow)


def hub_add(sp):
    st = STYLE[sp.name]
    s = Scene(sp, f"{sp.name.title()} adds a file to the Hub",
              f"Add to Hub is chosen from a file's right click menu; the notch opens into the Hub and the {sp.name} brings a copy of the file up into the empty slot.", 7, 56)
    icon = (88, 82)
    grab = (icon[0] + 6, icon[1] + 8)
    slot = TILES[2] + 10
    panel = hub_panel(s, [(0, 0, STEP), (22, 0, EASE_OUT), (27, 1, STEP), (84, 1, EASE_IN_OUT), (89, 0, STEP)],
                      tile(TILES[0], file_icon(TILES[0] + 4, 16, "text")) + tile(TILES[1], file_icon(TILES[1] + 4, 16, "zip")) +
                      f'<rect x="{TILES[2]}.5" y="14.5" width="19" height="19" fill="none" stroke="#C2C3C7" stroke-dasharray="2 2"/>' +
                      shown(s, [(68, 1), (90, 0)], s.anim(Track([(0, {"x": 0, "y": 0, "s": 1}), (68, {"x": slot * .4, "y": 24 * .4, "s": .6}, SPRING),
                                                                 (72, {"x": 0, "y": 0, "s": 1})]), tile(TILES[2], file_icon(TILES[2] + 4, 16, "picture")), "z")))
    stage = ['<rect width="240" height="150" fill="#1D2B53"/>', closed_notch(s), window(),
             file_icon(48, 82, "text"), file_icon(icon[0], icon[1], "picture"), file_icon(128, 82, "zip"), file_icon(168, 82, "pdf"),
             menu(s, 95, 89, ["Open", "Copy", "Add to Hub"], 2, "#FF77A8", "#000000", 11, 20), panel, badge(s, 68, 184)]
    arrow = pointer(s, [(0, {"x": 196, "y": 140, "o": 1}, EASE_IN_OUT), (8, {"x": 95, "y": 89}, STEP), (10, {"x": 96, "y": 90}, STEP),
                        (11, {"x": 95, "y": 89}, EASE_IN_OUT), (14, {"x": 104, "y": 105}, STEP), (19, {"x": 105, "y": 106}, STEP),
                        (20, {"x": 104, "y": 105}, EASE_IN_OUT), (28, {"x": 196, "y": 140}, STEP), (29, {"o": 0}, STEP), (97, {"o": 1}, STEP)])

    reach = st["reach"]
    low = grab[1] - reach  # body centre while it holds the file at arm's (or tongue's, or beam's) length
    party = PANEL_Y + 18
    where = Track([(0, {"x": HOME[0], "y": HOME[1]}, STEP), (22, {}, EASE_OUT), (27, {"y": OPEN_HOME[1]}, STEP),
                   (28, {}, "steps(8)"), (33, {"x": grab[0]}, EASE_IN_OUT), (36, {}, "steps(5)"), (42, {"y": low}, STEP),
                   (48, {}, EASE_OUT), (50, {"y": low - 4}, STEP), (53, {}, EASE_OUT), (55, {"y": low - 8}, STEP),
                   (58, {}, "steps(10)"), (63, {"x": slot}, EASE_IN_OUT), (66, {"y": PANEL_Y + 14}, EASE_OUT), (70, {"y": party}, STEP),
                   (72, {}, STEP), (74, {"y": party - 2}, STEP), (76, {"y": party}, STEP), (80, {}, EASE_IN_OUT),
                   (84, {"x": OPEN_HOME[0], "y": OPEN_HOME[1]}, EASE_IN_OUT), (89, {"y": HOME[1]}, STEP)])
    panel_edge = lambda p: NOTCH_Y if p < 22 or p >= 89 else PANEL_Y if 27 <= p < 84 else (NOTCH_Y + PANEL_Y) / 2
    ease = lambda p: where.keys[[k[0] for k in where.keys].index(p)][2]
    carry = (48, 50, 53, 55, 58, 63, 66)
    copy = Track([(0, {"x": grab[0], "y": grab[1], "s": 1, "o": 0}, STEP), (44, {"o": 1}, STEP)] +
                 follow(where, [(p, ease(p)) for p in carry], (0, reach)) +
                 [(68, {"x": slot, "y": PANEL_Y - 12, "s": .8}, STEP), (69, {"o": 0}, STEP)])
    payload = clipped(s, PANEL_Y, s.anim(copy, f'<g transform="translate(-6 -8)">{file_icon(0, 0, "picture")}</g>', "c"))
    strike = st.get("strike", st["ray"])
    rays = []
    if strike != st["ray"]:
        rays.append(ray(s, [(0, {**seg(at(where, 42), grab), "o": 0}, STEP), (42, {"o": 1}, STEP), (43, {"o": .4}, STEP), (44, {"o": 0}, STEP)], strike))
    rkeys = [(0, {**seg(at(where, 42), grab), "l": 0, "o": 0}, STEP), (42, {"o": 1}, EASE_OUT), (44, seg(at(where, 44), grab), STEP)]
    rkeys += [(p, seg(at(where, p), at(copy, p)), ease(p)) for p in carry] + [(68, {"l": 0, "o": 0}, STEP)]
    rays.append(ray(s, rkeys, st["ray"], 9))
    stick = Track([(0, {"x": grab[0], "y": grab[1] - 8, "o": 0}, STEP), (44, {"o": 1}, STEP)] +
                  follow(where, [(p, ease(p)) for p in carry], (0, reach - 8)) + [(67, {"o": 0}, STEP)])
    frames = [(0, "idle"), (4, "blink"), (5.5, "idle"), (12, "look-left"), (22, "alert"), (36, "look-left"), (42, "alert"),
              (48, "pull-a"), (50, "pull-b"), (53, "pull-a"), (55, "pull-b"), (58, "alert"), (68, "happy"), (84, "idle"),
              (95, "blink"), (96.5, "idle")]
    actor = ([dragline(s, where, panel_edge)] if st.get("hang") else []) + rays + [tip(s, stick, strike), frames_of(s, frames, where)]
    return s.svg("".join(stage), payload, "".join(actor), arrow)


def shield(s, kind, moved, keys_at, sprite_bottom):
    """What guards the file on its way down, riding along with it."""
    if kind == "rect":
        shape = '<rect x="-9.5" y="-11.5" width="19" height="23" fill="#29ADFF" fill-opacity=".15" stroke="#29ADFF"/>'
    elif kind == "bubble":
        shape = '<circle r="13" fill="#FFF1E8" fill-opacity=".25" stroke="#83769C"/>'
    elif kind == "circle":
        shape = '<circle r="14" fill="#29ADFF" fill-opacity=".1" stroke="#29ADFF" stroke-dasharray="3 2"/>'
    else:  # the saucer's beam, from its belly down over the file
        top = -26 + sprite_bottom
        shape = (f'<path fill="#FFEC27" fill-opacity=".3" d="M-3 {top}H3L11 12H-11Z"/>'
                 f'<path fill="#FFF1E8" fill-opacity=".25" d="M-1 {top}H1L7 12H-7Z"/>')
    spin = kind == "circle"
    keys = [(0, {"x": at(moved, 20)[0], "y": at(moved, 20)[1], "o": 0, **({"r": 0} if spin else {})}, STEP),
            (20, {"o": 1}, "linear")]
    keys += follow(moved, keys_at, extra=(lambda p: {"r": (p - 20) * 9}) if spin else None)
    keys.append((60, {"o": 0}, STEP))
    return s.anim(Track(keys), shape, "g")


def hub_drop(sp, variant):
    st = STYLE[sp.name]
    guard = variant == "guard"
    s = Scene(sp, f"{sp.name.title()} {'guards' if guard else 'lets go of'} a file dragged out of the Hub",
              (f"A file is dragged from the Hub into a mail window; the {sp.name} escorts it down behind a shield and salutes when it lands."
               if guard else
               f"A file is dragged from the Hub into a mail window; the {sp.name} holds on until it slips away, then waves goodbye with a tear."),
              7, 40 if guard else 44)
    zone = (82, 96, 76, 34)
    lift = (TILES[1] + 10, 24)
    land = (zone[0] + zone[2] // 2, zone[1] + zone[3] // 2)
    # The file follows the pointer exactly; the pointer's tip sits at the file's centre.
    drag = [(14, lift), (17, (lift[0], lift[1] - 1)), (30, (98, 58)), (44, (108, 88)), (58, land)]
    file_keys = [(0, {"x": lift[0], "y": lift[1], "o": 0}, STEP), (17, {"o": 1}, "linear")]
    for p, (x, y) in drag[2:]:
        file_keys.append((p, {"x": x, "y": y}, EASE_IN_OUT if p == 58 else "linear"))
    file_keys += [(60, {"o": 0}, STEP)]
    moved = Track(file_keys)
    panel = hub_panel(s, [(0, 0, STEP), (6, 0, EASE_OUT), (10, 1, STEP), (84, 1, EASE_IN_OUT), (89, 0, STEP)],
                      tile(TILES[0], file_icon(TILES[0] + 4, 16, "text")) + tile(TILES[1]) + tile(TILES[2], file_icon(TILES[2] + 4, 16, "zip")) +
                      shown(s, [(0, 1), (17, 0), (92, 1)], s.anim(Track([(0, {"y": 0}, STEP), (14, {"y": -1}, STEP), (17, {"y": 0}, STEP)]),
                                                                 file_icon(TILES[1] + 4, 16, "picture"), "u")))
    hot = shown(s, [(50, 1), (60, 0)], f'<rect x="{zone[0]}.5" y="{zone[1]}.5" width="{zone[2] - 1}" height="{zone[3] - 1}" fill="#00E436" fill-opacity=".15" stroke="#00E436" stroke-dasharray="2 2"/>')
    chip = shown(s, [(60, 1), (94, 0)], s.anim(Track([(0, {"s": 1}), (60, {"x": land[0] * .3, "y": land[1] * .3, "s": .7}, SPRING), (64, {"x": 0, "y": 0, "s": 1})]),
                                               f'<rect x="{land[0] - 20}" y="{land[1] - 6}" width="40" height="12" fill="#C2C3C7"/>' +
                                               file_icon(land[0] - 18, land[1] - 8, "picture") +
                                               f'<rect x="{land[0] - 4}" y="{land[1] - 2}" width="20" height="2" fill="#5F574F"/>'
                                               f'<rect x="{land[0] - 4}" y="{land[1] + 2}" width="12" height="1" fill="#83769C"/>', "u"))
    stage = ['<rect width="240" height="150" fill="#1D2B53"/>', closed_notch(s), window(),
             '<text x="31" y="79" font-size="5" fill="#5F574F">To:</text><rect x="44" y="76" width="60" height="2" fill="#83769C"/>',
             '<rect x="31" y="84" width="178" height="1" fill="#C2C3C7"/>',
             f'<rect x="{zone[0]}.5" y="{zone[1]}.5" width="{zone[2] - 1}" height="{zone[3] - 1}" fill="none" stroke="#5F574F" stroke-dasharray="2 2"/>',
             f'<text x="{land[0] - 13}" y="{land[1] + 2}" font-size="5" fill="#5F574F">Drop file</text>', hot, chip, panel]
    arrow_keys = [(0, {"x": 200, "y": 140, "o": 1}, EASE_IN_OUT), (6, {"x": 120, "y": 4}, STEP), (8, {}, EASE_IN_OUT),
                  (12, {"x": lift[0], "y": lift[1]}, STEP), (14, {"x": lift[0] + 1, "y": lift[1] + 1}, STEP)]
    for p, (x, y) in drag[1:]:
        arrow_keys.append((p, {"x": x + 1, "y": y + 1}, EASE_IN_OUT if p == 58 else "linear"))
    arrow_keys[-1] = (58, arrow_keys[-1][1], STEP)
    arrow_keys += [(60, {"x": land[0], "y": land[1]}, EASE_IN_OUT), (70, {"x": 200, "y": 140}, STEP), (71, {"o": 0}, STEP), (98, {"o": 1}, STEP)]
    arrow = pointer(s, arrow_keys)
    payload = s.anim(moved, f'<g transform="translate(-6 -8)">{file_icon(0, 0, "picture")}</g>', "c")
    panel_edge = lambda p: NOTCH_Y if p < 6 or p >= 89 else PANEL_Y if 10 <= p < 84 else (NOTCH_Y + PANEL_Y) / 2
    hang = [dragline] if st.get("hang") else []

    if guard:
        side = st["side"]
        keys = [(0, {"x": HOME[0], "y": HOME[1]}, STEP), (6, {}, EASE_OUT), (10, {"y": OPEN_HOME[1]}, STEP), (17, {}, EASE_OUT)]
        keys += follow(moved, [(22, "linear"), (30, "linear"), (44, EASE_IN_OUT), (58, EASE_OUT)], side)
        aside = (land[0] - 34, at(moved, 58)[1] + side[1]) if side[0] else (land[0] - 30, land[1] - 6)
        keys += [(61, {"x": aside[0], "y": aside[1]}, STEP), (72, {}, "steps(12)"), (80, {"x": OPEN_HOME[0], "y": OPEN_HOME[1]}, STEP),
                 (84, {}, EASE_IN_OUT), (89, {"y": HOME[1]}, STEP)]
        where = Track(keys)
        guard_el = shield(s, st["shield"], moved, [(30, "linear"), (44, EASE_IN_OUT), (58, STEP)], sp.bottom)
        frames = [(0, "idle"), (4, "blink"), (5.5, "idle"), (12, "alert"), (20, "look-right"), (26, "look-left"), (32, "look-right"),
                  (38, "look-left"), (44, "look-right"), (50, "look-left"), (56, "alert"), (62, "proud"), (74, "happy"), (80, "idle"),
                  (95, "blink"), (96.5, "idle")]
        actor = [h(s, where, panel_edge) for h in hang] + [guard_el, frames_of(s, frames, where)]
    else:
        snap = 54
        tugs = [(22, 0), (26, 4), (28, 2), (33, 7), (35, 5), (40, 10), (42, 8), (47, 13), (49, 11), (snap, 11)]
        keys = [(0, {"x": HOME[0], "y": HOME[1]}, STEP), (6, {}, EASE_OUT), (10, {"y": OPEN_HOME[1]}, STEP)]
        for p, d in tugs:
            keys.append((p, {"y": OPEN_HOME[1] + d}, EASE_OUT))
        keys += [(snap + 1.5, {"y": OPEN_HOME[1] - 6}, EASE_IN_OUT), (snap + 5, {"y": OPEN_HOME[1]}, STEP),
                 (84, {}, EASE_IN_OUT), (89, {"y": HOME[1]}, STEP)]
        where = Track(keys)
        mouth = lambda p: (at(where, p)[0], at(where, p)[1] + (sp.bottom - 2 if st["ray"] == "beam" else 0))
        tether = [(0, {**seg(mouth(17), lift), "l": 0, "o": 0}, STEP), (14, {"o": 1}, EASE_OUT), (17, seg(mouth(17), lift), "linear")]
        tether += [(p, seg(mouth(p), at(moved, p)), "linear") for p in range(18, snap)]
        tether.append((snap, {**seg(mouth(snap), at(moved, snap)), "o": 0}, STEP))
        a, b = mouth(snap), at(moved, snap)
        piece = lambda u, v: seg(u, (u[0] + (v[0] - u[0]) * .35, u[1] + (v[1] - u[1]) * .35))
        recoil_a = ray(s, [(0, {**piece(a, b), "o": 0}, STEP), (snap, {"o": 1}, EASE_OUT), (snap + 5, {"l": 1, "o": 0}, STEP)], st["ray"], 9)
        recoil_b = ray(s, [(0, {**piece(b, a), "o": 0}, STEP), (snap, {"o": 1}, EASE_OUT), (snap + 5, {"l": 1, "o": 0}, STEP)], st["ray"], 9)
        frames = [(0, "idle"), (4, "blink"), (5.5, "idle"), (12, "alert"), (18, "pull-a"), (26, "pull-b"), (28, "pull-a"),
                  (33, "pull-b"), (35, "pull-a"), (40, "pull-b"), (42, "pull-a"), (47, "pull-b"), (49, "pull-a"),
                  (snap, "alert"), (57, "look-left"), (61, "sad"),
                  (66, "wave-a"), (69, "sad"), (72, "wave-a"), (75, "sad"), (78, "wave-a"), (81, "sad"), (90, "idle"),
                  (95, "blink"), (96.5, "idle")]
        actor = ([h(s, where, panel_edge) for h in hang] + [ray(s, tether, st["ray"], 9), recoil_a, recoil_b,
                 frames_of(s, frames, where), s.anim(where, tear(s, [61, 70]), "w")])
    return s.svg("".join(stage), payload, "".join(actor), arrow)


SCENES = {"copy-text": lambda sp: copy_scene(sp, "text"), "copy-image": lambda sp: copy_scene(sp, "image"),
          "hub-add": hub_add, "hub-drop-guard": lambda sp: hub_drop(sp, "guard"),
          "hub-drop-regret": lambda sp: hub_drop(sp, "regret")}


def build(name):
    sp = Sprite(name)
    for scene, make in SCENES.items():
        out = HERE / f"{name}-{scene}.svg"
        out.write_text(make(sp))
        print(f"wrote {out.name} ({out.stat().st_size} bytes)")


def check():
    import xml.etree.ElementTree as ET
    with tempfile.TemporaryDirectory() as d:
        grid = [[0, -1, 7], [12, 8, -1]]
        write_png(f"{d}/t.png", grid)
        w, h, px = read_png(f"{d}/t.png")
        assert [[classify(p) for p in row] for row in px] == grid
        # `compose` on a sheet of loose sprites: twelve sprites of four sizes in three rows of four,
        # each in the colour of its column, so what comes out can be counted, measured and placed.
        sizes = [(18, 17), (16, 19), (17, 18), (20, 16), (19, 17), (16, 18), (17, 19), (18, 16),
                 (20, 18), (16, 17), (19, 18), (17, 16)]
        unit, cell_dim = 3, 24  # one art pixel in the sheet is three image pixels; cells of 24 x 24 art pixels
        loose = [[-1] * (cell_dim * 4 * unit) for _ in range(cell_dim * 3 * unit)]
        for k, (sw, sh) in enumerate(sizes):
            col = 8 + k % 4
            for y in range(sh * unit):
                for x in range(sw * unit):
                    c = col if ((x // unit) + (y // unit)) % 2 == 0 else 0
                    loose[(k // 4) * cell_dim * unit + (cell_dim - sh) // 2 * unit + y][(k % 4) * cell_dim * unit + (cell_dim - sw) // 2 * unit + x] = c
        write_png(f"{d}/loose.png", loose)
        compose(f"{d}/loose.png", "check", art=pathlib.Path(d))
        cw, ch, cut = read_png(f"{d}/check.png")
        assert (cw, ch) == (CELL * 12, CELL), (cw, ch)
        for k, (sw, sh) in enumerate(sizes):
            ink = {(x - k * CELL, y): classify(cut[y][x]) for y in range(CELL) for x in range(k * CELL, (k + 1) * CELL)}
            ink = {p: v for p, v in ink.items() if v >= 0}
            xs, ys = [x for x, _ in ink], [y for _, y in ink]
            assert len(ink) == sw * sh, f"frame {k}: {len(ink)} pixels, wanted {sw * sh}"
            assert (max(xs) - min(xs) + 1, max(ys) - min(ys) + 1) == (sw, sh), f"frame {k} is the wrong size: {xs} x {ys}"
            assert set(ink.values()) == {8 + k % 4, 0}, f"frame {k} is not the {k}th sprite"
    t = Track([(0, {"x": 0}), (50, {"x": 10}, STEP), (60, {"x": 20})])
    assert (t.at(25)["x"], t.at(55)["x"], t.at(99)["x"]) == (5, 10, 20)
    bad = ("<animate", "<set", "<script", "foreignObject", "<image", "filter", "mask", "http://", "https://")
    files = sorted(HERE.glob("*.svg"))
    for f in files:
        text = f.read_text()
        ET.fromstring(text)
        body = text.replace('xmlns="http://www.w3.org/2000/svg"', "")
        assert not [b for b in bad if b in body], f"{f.name}: {[b for b in bad if b in body]}"
        assert len(set(re.findall(r"animation-duration:([\d.]+s)", text))) == 1, f"{f.name}: more than one loop length"
        assert text.isascii(), f"{f.name}: non-ASCII text"
    print(f"check passed: {len(files)} SVG files")


if __name__ == "__main__":
    if sys.argv[1:2] == ["extract"]:
        extract(sys.argv[2], sys.argv[3])
    elif sys.argv[1:2] == ["compose"]:
        compose(sys.argv[2], sys.argv[3])
    elif sys.argv[1:2] == ["check"]:
        check()
    else:
        names = sys.argv[1:] or [png.stem for png in sorted(ART.glob("*.png"))]
        for name in names:
            build(name)
