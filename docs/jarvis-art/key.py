import sys, pathlib, importlib.util, collections
spec = importlib.util.spec_from_file_location("build", "/Users/bdthai/Desktop/figghost/docs/mascots/build.py")
b = importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
w, h, px = b.read_png(sys.argv[1])
dark = lambda p: p[0] < 45 and p[1] < 45 and p[2] < 45
seen = [[False]*w for _ in range(h)]
q = collections.deque((x, y) for x in range(w) for y in (0, h-1)) 
q.extend((x, y) for y in range(h) for x in (0, w-1))
while q:
    x, y = q.popleft()
    if not (0 <= x < w and 0 <= y < h) or seen[y][x] or not dark(px[y][x]): continue
    seen[y][x] = True
    q.extend(((x+1,y),(x-1,y),(x,y+1),(x,y-1)))
# write RGBA with magenta where background
import zlib, struct
rows = b"".join(b"\0" + bytes(c for x in range(w) for c in ((255,0,255,255) if seen[y][x] else tuple(px[y][x][:3])+(255,))) for y in range(h))
def chunk(k, d): return struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d) & 0xffffffff)
pathlib.Path(sys.argv[2]).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
b.compose(sys.argv[2], "jarvis", art=pathlib.Path(sys.argv[3]))
