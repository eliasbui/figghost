// Draws the ghost of the menu bar item as src-tauri/icons/tray.png: the same path as `ghost_path` in
// crates/macos-sys/src/status_item.rs, black with the shape in the alpha channel, which the menu bar reads as a template
// image. Run it with node when the ghost changes; the output is a file of the repository.
import { writeFileSync } from 'node:fs';
import { crc32, deflateSync } from 'node:zlib';

const PIXELS_PER_POINT = 2;
const WIDTH = 16 * PIXELS_PER_POINT;
const HEIGHT = 18 * PIXELS_PER_POINT;
// Points per unit of the 24 by 24 box the path is drawn in, and where the box is put (status_item.rs: ICON_SCALE).
const UNIT = 14.5 / 17;
const SAMPLES = 8;

const at = (x, y) => [x, y];
// Each curve ends at the first point, then control points 1 and 2, as `curveToPoint_controlPoint1_controlPoint2`.
const CURVES = [
  [at(5.0, 9.2), at(8.0, 2.5), at(5.0, 5.8)],
  [at(5.8, 17.0), at(5.0, 12.0), at(5.5, 14.8)],
  [at(8.0, 19.5), at(6.0, 18.2), at(7.0, 19.5)],
  [at(10.2, 17.8), at(9.0, 19.5), at(9.8, 18.5)],
  [at(12.5, 19.3), at(10.8, 18.8), at(11.8, 19.3)],
  [at(14.8, 17.6), at(13.5, 19.3), at(14.2, 18.2)],
  [at(20.5, 14.5), at(15.5, 18.5), at(18.2, 17.8)],
  [at(19.4, 12.8), at(21.2, 13.8), at(20.4, 12.8)],
  [at(18.3, 9.2), at(18.6, 11.8), at(18.2, 10.5)],
  [at(11.5, 2.5), at(18.3, 5.8), at(15.2, 2.5)],
];
const EYES = [
  [7.6, 8.4, 2.6, 3.8],
  [12.6, 8.4, 2.6, 3.8],
];

const bezier = (p0, p1, c1, c2, t) => {
  const u = 1 - t;
  return [0, 1].map((i) => u ** 3 * p0[i] + 3 * u * u * t * c1[i] + 3 * u * t * t * c2[i] + t ** 3 * p1[i]);
};

const ghost = [at(11.5, 2.5)];
let from = ghost[0];
for (const [end, c1, c2] of CURVES) {
  for (let step = 1; step <= 24; step++) ghost.push(bezier(from, end, c1, c2, step / 24));
  from = end;
}
const eyes = EYES.map(([x, y, w, h]) =>
  Array.from({ length: 48 }, (_, i) => [x + w / 2 + (w / 2) * Math.cos((i / 48) * 2 * Math.PI), y + h / 2 + (h / 2) * Math.sin((i / 48) * 2 * Math.PI)]),
);

// Unit box to pixels: the ghost is centered across the icon and its top is 1.5 points from the top edge.
const place = ([x, y]) => [((x - 12.75) * UNIT + 8) * PIXELS_PER_POINT, ((y - 2.5) * UNIT + 1.5) * PIXELS_PER_POINT];
const polygons = [ghost, ...eyes].map((ring) => ring.map(place));

// Even-odd rule: a point is inside when a ray to the right crosses an odd number of edges, eyes included.
const inside = (x, y) => {
  let crossings = 0;
  for (const ring of polygons) {
    for (let i = 0; i < ring.length; i++) {
      const [x1, y1] = ring[i];
      const [x2, y2] = ring[(i + 1) % ring.length];
      if (y1 > y !== y2 > y && x < x1 + ((y - y1) / (y2 - y1)) * (x2 - x1)) crossings++;
    }
  }
  return crossings % 2 === 1;
};

const raw = Buffer.alloc((WIDTH * 4 + 1) * HEIGHT);
for (let py = 0; py < HEIGHT; py++) {
  const row = py * (WIDTH * 4 + 1);
  raw[row] = 0;
  for (let px = 0; px < WIDTH; px++) {
    let covered = 0;
    for (let sy = 0; sy < SAMPLES; sy++) for (let sx = 0; sx < SAMPLES; sx++) if (inside(px + (sx + 0.5) / SAMPLES, py + (sy + 0.5) / SAMPLES)) covered++;
    raw[row + 1 + px * 4 + 3] = Math.round((covered / (SAMPLES * SAMPLES)) * 255);
  }
}

const chunk = (type, data) => {
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const size = Buffer.alloc(4);
  size.writeUInt32BE(data.length);
  const sum = Buffer.alloc(4);
  sum.writeUInt32BE(crc32(body));
  return Buffer.concat([size, body, sum]);
};
const header = Buffer.alloc(13);
header.writeUInt32BE(WIDTH, 0);
header.writeUInt32BE(HEIGHT, 4);
header[8] = 8; // bit depth
header[9] = 6; // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', header),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);
const out = new URL('../src-tauri/icons/tray.png', import.meta.url);
writeFileSync(out, png);
console.log(`${out.pathname}: ${WIDTH}x${HEIGHT}, ${png.length} bytes`);
