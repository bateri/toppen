// Draws toppen's icon, its mark (four bars of a meter, the last one the tallest), as PNGs, and
// writes them to src/assets/icon-<side>-<theme>.png. A terminal draws a program's icon only from
// the bytes it is given, so the program carries them (src/program/icons.rs names the files); run
// this again after changing the shapes here.
//
//   node tools/icon.mjs
//
// It uses nothing but Node: the shapes are rounded rectangles, filled with coverage taken
// from a grid of samples in each pixel, and the PNG is written by hand around zlib.

import { writeFileSync } from "node:fs";
import { deflateSync, crc32 } from "node:zlib";

/** The mark in its 24-unit box: x, y, width, height, corner radius, opacity. */
const MARK = [
  [2.9, 13.3, 3.2, 7, 1.6, 0.4],
  [7.9, 7.8, 3.2, 12.5, 1.6, 0.7],
  [12.9, 15.3, 3.2, 5, 1.6, 0.4],
  [17.9, 4.3, 3.2, 16, 1.6, 1],
];

/** The part of that box an icon shows: the mark (x 2.9–21.1, y 4.3–20.3) with a unit around it. */
const FRAME = { x: 2, y: 2, side: 20 };

/** One colour for each ground, as an icon's `theme` says: the ground the icon is made to stand on. */
const COLOURS = {
  dark: [0x6e, 0xd0, 0xa8],
  light: [0x0f, 0x7a, 0x5a],
};

/** 32 pixels is what a terminal's strip asks for on a Retina screen; 64 for those that draw larger. */
const SIDES = [32, 64];

const SAMPLES = 8;

function inside(px, py, [x, y, w, h, r]) {
  const dx = Math.max(Math.abs(px - (x + w / 2)) - (w / 2 - r), 0);
  const dy = Math.max(Math.abs(py - (y + h / 2)) - (h / 2 - r), 0);
  return dx * dx + dy * dy <= r * r;
}

function draw(side, [red, green, blue]) {
  const scale = FRAME.side / side;
  const rows = [];
  for (let row = 0; row < side; row++) {
    const line = Buffer.alloc(1 + side * 4); // a leading 0: no filter
    for (let column = 0; column < side; column++) {
      // The shapes are drawn one over another, each with its opacity.
      let alpha = 0;
      for (const shape of MARK) {
        let hits = 0;
        for (let sy = 0; sy < SAMPLES; sy++) {
          for (let sx = 0; sx < SAMPLES; sx++) {
            const px = FRAME.x + (column + (sx + 0.5) / SAMPLES) * scale;
            const py = FRAME.y + (row + (sy + 0.5) / SAMPLES) * scale;
            if (inside(px, py, shape)) hits++;
          }
        }
        const cover = (hits / (SAMPLES * SAMPLES)) * shape[5];
        alpha = cover + alpha * (1 - cover);
      }
      line.set([red, green, blue, Math.round(alpha * 255)], 1 + column * 4);
    }
    rows.push(line);
  }
  return png(side, Buffer.concat(rows));
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function png(side, pixels) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(side, 0);
  header.writeUInt32BE(side, 4);
  header.set([8, 6, 0, 0, 0], 8); // 8 bits, RGBA, deflate, no filter method, no interlace
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(pixels, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

let written = 0;
for (const [theme, colour] of Object.entries(COLOURS)) {
  for (const side of SIDES) {
    writeFileSync(
      new URL(`../src/assets/icon-${side}-${theme}.png`, import.meta.url),
      draw(side, colour),
    );
    written++;
  }
}
console.log(`wrote ${written} icons to src/assets`);
