import sys, pathlib, importlib.util, collections, zlib, struct
spec = importlib.util.spec_from_file_location("b", "/Users/bdthai/Desktop/figghost/docs/mascots/build.py")
b = importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
w, h, px = b.read_png(sys.argv[1])
bg = lambda p: p[0] > 250 and p[1] < 5 and p[2] > 250
CELL = int(sys.argv[3])
frames = []
for k in range(12):
    c, r = k % 4, k // 4
    x0, x1, y0, y1 = w*c//4, w*(c+1)//4, h*r//3, h*(r+1)//3
    seen = set(); best = []
    for y in range(y0, y1, 2):
        for x in range(x0, x1, 2):
            if (x, y) in seen or bg(px[y][x]): continue
            comp = []; q = collections.deque([(x, y)]); seen.add((x, y))
            while q:
                a, d = q.popleft(); comp.append((a, d))
                for n in ((a+1,d),(a-1,d),(a,d+1),(a,d-1)):
                    if x0 <= n[0] < x1 and y0 <= n[1] < y1 and n not in seen and not bg(px[n[1]][n[0]]):
                        seen.add(n); q.append(n)
            if len(comp) > len(best): best = comp
    xs = [p[0] for p in best]; ys = [p[1] for p in best]
    cx, cy = (min(xs)+max(xs))//2, (min(ys)+max(ys))//2
    frames.append((cx, cy))
out = []
for y in range(CELL):
    row = bytearray(b"\0")
    for (cx, cy) in frames:
        for x in range(CELL):
            sx, sy = cx - CELL//2 + x, cy - CELL//2 + y + 8
            if 0 <= sx < w and 0 <= sy < h and not bg(px[sy][sx]):
                row += bytes(px[sy][sx][:3]) + b"\xff"
            else:
                row += b"\0\0\0\0"
    out.append(bytes(row))
chunk = lambda k, d: struct.pack(">I", len(d)) + k + d + struct.pack(">I", zlib.crc32(k + d))
pathlib.Path(sys.argv[2]).write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", CELL*12, CELL, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(b"".join(out), 9)) + chunk(b"IEND", b""))
print(frames)
