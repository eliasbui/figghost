#!/usr/bin/env python3
"""Pixel sprites for the Vietnamese typing animation and language badge shown by the closed notch and menu bar
(crates/app-ui/src/features/typing/mod.rs).

  build.py          write both sheets into crates/app-ui/assets/typing/
  build.py check    self-test (builds into a temporary folder that is removed afterwards, also on failure)

vietnam.png is a row of 8 square 24 x 24 frames of the national flag of Vietnam waving on a flagpole.
flags.png is a row of 2 square 16 x 16 frames (Vietnamese, English) with rounded corners for the status badge.
"""
import math, pathlib, struct, sys, tempfile, zlib

HERE = pathlib.Path(__file__).resolve().parent
ART = HERE.parent.parent / "crates" / "app-ui" / "assets" / "typing"

VN_CELL, VN_FRAMES = 24, 8
FLAG_CELL, FLAG_FRAMES = 16, 2
FLAG_TOP, FLAG_BOTTOM = 2, 12


def rgb(hex_colour):
    return tuple(int(hex_colour[i:i + 2], 16) for i in (1, 3, 5))


INK = {
    # Vietnamese flag reds: base field, two upward-slope highlights, and trough shadow.
    "r": "#DA251D",
    "l": "#F0453A",
    "h": "#FF6B5E",
    "s": "#A3150F",
    # Golds for the star, its lower-edge depth rim, the finial, and festive sparkles.
    "y": "#FFDD00",
    "d": "#E0A800",
    "k": "#FFF6B0",
    # Flagpole light grey shaft and left highlight column.
    "p": "#C8C8CC",
    "P": "#ECECF0",
    # United States badge palette: stripes, white stars/stripes, and canton blue.
    "R": "#B22234",
    "W": "#F4F4F4",
    "B": "#3C3B6E",
}
PALETTE = {k: rgb(v) for k, v in INK.items()}
RED_FAMILY = {PALETTE[k] for k in ("r", "l", "h", "s")}
GOLD_FAMILY = {PALETTE[k] for k in ("y", "d", "k")}


def stamp(art, x0=0, y0=0):
    """A drawing in the letters of INK, any other character clear, with its top left at (x0, y0)."""
    return {
        (x0 + x, y0 + y): PALETTE[ch]
        for y, line in enumerate(art.split())
        for x, ch in enumerate(line)
        if ch in PALETTE
    }


def write_rgba(path, rows):
    """rows of (r, g, b), or None where clear."""
    raw = b"".join(b"\x00" + b"".join(bytes(c + (255,)) if c else b"\x00" * 4 for c in row) for row in rows)
    chunk = lambda k, d: struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d))
    pathlib.Path(path).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(
        ">IIBBBBB", len(rows[0]), len(rows), 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def read_png(path):
    """Reads an 8-bit colour-type-6 PNG written by write_rgba back into (width, height, rgba_rows)."""
    data = pathlib.Path(path).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, w, h, idat = 8, 0, 0, bytearray()
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + length]
        assert zlib.crc32(kind + body) == struct.unpack(">I", data[pos + 8 + length:pos + 12 + length])[0]
        pos += 12 + length
        if kind == b"IHDR":
            w, h, depth, ctype, comp, filt, inter = struct.unpack(">IIBBBBB", body)
            assert (depth, ctype, comp, filt, inter) == (8, 6, 0, 0, 0)
        elif kind == b"IDAT":
            idat.extend(body)
        elif kind == b"IEND":
            break
    raw = zlib.decompress(bytes(idat))
    stride = 1 + 4 * w
    assert len(raw) == h * stride
    rows = []
    for y in range(h):
        line = raw[y * stride:(y + 1) * stride]
        assert line[0] == 0
        rows.append([tuple(line[1 + 4 * x:1 + 4 * (x + 1)]) for x in range(w)])
    return w, h, rows


def pack_cells(frames, side, path):
    """Packs fixed-coordinate (x, y) -> rgb dictionaries into a horizontal strip of side x side cells."""
    rows = [[None] * (side * len(frames)) for _ in range(side)]
    for k, frame in enumerate(frames):
        for (x, y), colour in frame.items():
            assert 0 <= x < side and 0 <= y < side, (k, x, y)
            rows[y][k * side + x] = colour
    path.parent.mkdir(parents=True, exist_ok=True)
    write_rgba(path, rows)
    print(f"wrote {path} ({len(frames)} frames of {side} x {side})")


# Seven-pixel-wide five-pointed star with a 1-pixel darker gold (#E0A800) rim along every lower edge.
WAVING_STAR = stamp("""
    ...y...
    ...y...
    yyyyyyy
    .dyyyd.
    ..ydy..
    .yd.dy.
    .d...d.
""", 5, 2)

# Sparkles near the upper free corner appear in selected frames so the wave feels festive without clutter.
PLUS_SPARKLE = ".k. kkk .k."
SPARKLES = {
    1: stamp("k", 20, 2),
    2: stamp(PLUS_SPARKLE, 19, 1),
    3: {**stamp("k", 20, 2), **stamp("k", 22, 4)},
    5: stamp("k", 19, 2),
    6: {**stamp(PLUS_SPARKLE, 18, 1), **stamp("k", 22, 3)},
    7: stamp("k", 22, 3),
}


def pole():
    """Stationary flagpole in columns 2..3 with a 2 x 2 gold ball finial at the top."""
    out = stamp("""
        ky
        yd
    """, 2, 2)
    for y in range(4, 22):
        out[(2, y)] = PALETTE["P"]
        out[(3, y)] = PALETTE["p"]
    return out


def vietnam_frame(frame):
    """One 24 x 24 frame of the 18 x 12 cloth rippling rightwards from the pole at x = 4."""
    out = pole()
    phase = 2.0 * math.pi * frame / VN_FRAMES
    wavelength = 12.0
    cloth_w, cloth_h = 18, 12
    cloth_x0, cloth_y0 = 4, 5
    for col in range(cloth_w):
        # Amplitude is 0 at col 0 so the left edge stays pinned to the pole, growing to 2 px at the fly end.
        amp = 2.0 * col / (cloth_w - 1)
        angle = 2.0 * math.pi * (col / wavelength) - phase
        dy = int(round(amp * math.sin(angle)))
        # Screen y grows downward, so a negative derivative dy/dcol means the cloth is rising toward the light.
        slope = -amp * (2.0 * math.pi / wavelength) * math.cos(angle)
        for row in range(cloth_h):
            star_px = WAVING_STAR.get((col, row))
            if star_px is not None:
                colour = star_px
            elif slope > 0.62 and row < 4:
                colour = PALETTE["h"]
            elif slope > 0.22:
                colour = PALETTE["l"]
            elif slope < -0.25:
                colour = PALETTE["s"]
            else:
                colour = PALETTE["r"]
            out[(cloth_x0 + col, cloth_y0 + row + dy)] = colour
    out.update(SPARKLES.get(frame, {}))
    return out


def vietnam_frames():
    return [vietnam_frame(k) for k in range(VN_FRAMES)]


FLAT_VIETNAM = stamp("""
    .rrrrrrrrrrrrrr.
    rrrrrrrrrrrrrrrr
    rrrrrrryrrrrrrrr
    rrrrrrryrrrrrrrr
    rrrryyyyyyyrrrrr
    rrrrryyyyyrrrrrr
    rrrrrryyyrrrrrrr
    rrrrryyryyrrrrrr
    rrrrryrrryrrrrrr
    rrrrrrrrrrrrrrrr
    .rrrrrrrrrrrrrr.
""", 0, FLAG_TOP)

FLAT_USA = stamp("""
    .BBBBBBBRRRRRRR.
    BWBWBWBRRRRRRRRR
    BBBBBBBWWWWWWWWW
    BWBWBWBRRRRRRRRR
    BBBBBBBRRRRRRRRR
    BBBBBBBWWWWWWWWW
    RRRRRRRRRRRRRRRR
    RRRRRRRRRRRRRRRR
    WWWWWWWWWWWWWWWW
    RRRRRRRRRRRRRRRR
    .RRRRRRRRRRRRRR.
""", 0, FLAG_TOP)


def flag_frames():
    return [FLAT_VIETNAM, FLAT_USA]


def build(art=ART):
    pack_cells(vietnam_frames(), VN_CELL, art / "vietnam.png")
    pack_cells(flag_frames(), FLAG_CELL, art / "flags.png")


def check():
    with tempfile.TemporaryDirectory() as tmp1, tempfile.TemporaryDirectory() as tmp2:
        d1, d2 = pathlib.Path(tmp1), pathlib.Path(tmp2)
        build(art=d1)
        build(art=d2)
        for name in ("vietnam.png", "flags.png"):
            assert (d1 / name).read_bytes() == (d2 / name).read_bytes(), f"{name} is not deterministic"

        vw, vh, vpx = read_png(d1 / "vietnam.png")
        assert (vw, vh) == (VN_CELL * VN_FRAMES, VN_CELL), (vw, vh)
        for row in vpx:
            for r, g, b, a in row:
                assert a in (0, 255), a
                if a == 0:
                    assert (r, g, b) == (0, 0, 0), (r, g, b)
                else:
                    assert max(r, g, b) >= 0x30, (r, g, b)

        cells = []
        pole_cols = []
        for k in range(VN_FRAMES):
            x0 = k * VN_CELL
            cell = tuple(tuple(vpx[y][x0:x0 + VN_CELL]) for y in range(VN_CELL))
            cells.append(cell)
            for i in range(VN_CELL):
                assert cell[0][i][3] == 0 and cell[VN_CELL - 1][i][3] == 0, f"frame {k} top/bottom margin at {i}"
                assert cell[i][0][3] == 0 and cell[i][VN_CELL - 1][3] == 0, f"frame {k} left/right margin at {i}"
            colours = {cell[y][x][:3] for y in range(VN_CELL) for x in range(VN_CELL) if cell[y][x][3] == 255}
            assert PALETTE["r"] in colours and (colours & RED_FAMILY), f"frame {k} missing red family"
            assert PALETTE["y"] in colours and PALETTE["d"] in colours and (colours & GOLD_FAMILY), f"frame {k} missing gold family"
            pole_slice = tuple((cell[y][2], cell[y][3]) for y in range(VN_CELL))
            assert any(p[0][3] == 255 and p[1][3] == 255 for p in pole_slice)
            pole_cols.append(pole_slice)

        assert len(set(cells)) == VN_FRAMES, "vietnam frames must all differ"
        assert len(set(pole_cols)) == 1, "pole columns must be identical across all frames"

        fw, fh, fpx = read_png(d1 / "flags.png")
        assert (fw, fh) == (FLAG_CELL * FLAG_FRAMES, FLAG_CELL), (fw, fh)
        for row in fpx:
            for r, g, b, a in row:
                assert a in (0, 255), a
                if a == 0:
                    assert (r, g, b) == (0, 0, 0), (r, g, b)

        flag_colours = []
        for k in range(FLAG_FRAMES):
            x0 = k * FLAG_CELL
            for cx, cy in ((x0, FLAG_TOP), (x0 + FLAG_CELL - 1, FLAG_TOP),
                           (x0, FLAG_BOTTOM), (x0 + FLAG_CELL - 1, FLAG_BOTTOM)):
                assert fpx[cy][cx][3] == 0, f"frame {k} corner {(cx - x0, cy)} not transparent"
            assert fpx[FLAG_TOP][x0 + 1][3] == 255 and fpx[FLAG_BOTTOM][x0 + 1][3] == 255
            colours = {fpx[y][x][:3] for y in range(FLAG_CELL) for x in range(x0, x0 + FLAG_CELL) if fpx[y][x][3] == 255}
            flag_colours.append(colours)

        assert PALETTE["y"] in flag_colours[0] and PALETTE["B"] not in flag_colours[0], flag_colours[0]
        assert PALETTE["B"] in flag_colours[1] and PALETTE["W"] in flag_colours[1], flag_colours[1]
        assert not (flag_colours[1] & GOLD_FAMILY), flag_colours[1]
    print("check passed")


if __name__ == "__main__":
    if sys.argv[1:] == []:
        build()
    elif sys.argv[1:] == ["check"]:
        check()
    else:
        sys.exit(__doc__)
