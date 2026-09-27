'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { createRgbaPng } = require('../helpers/png-helpers');

const FIXTURES_DIR = __dirname;
const JPEG_SOURCE = path.join(FIXTURES_DIR, 'test_100KB_1057x1057.jpg');

function createExifWithoutOrientation() {
  const tiff = Buffer.alloc(31);
  tiff.write('II', 0, 'ascii');
  tiff.writeUInt16LE(42, 2);
  tiff.writeUInt32LE(8, 4);
  tiff.writeUInt16LE(1, 8);
  tiff.writeUInt16LE(0x0131, 10); // Software
  tiff.writeUInt16LE(2, 12); // ASCII
  tiff.writeUInt32LE(5, 14);
  tiff.writeUInt32LE(26, 18);
  tiff.writeUInt32LE(0, 22);
  tiff.write('test\0', 26, 'ascii');

  const exif = Buffer.concat([Buffer.from('Exif\0\0', 'ascii'), tiff]);
  const app1 = Buffer.alloc(4 + exif.length);
  app1.writeUInt16BE(exif.length + 2, 2);
  app1[0] = 0xff;
  app1[1] = 0xe1;
  exif.copy(app1, 4);
  return app1;
}

function createOrientationAbsentJpeg() {
  const source = fs.readFileSync(JPEG_SOURCE);
  if (source[0] !== 0xff || source[1] !== 0xd8) throw new Error('JPEG source must start with SOI');
  return Buffer.concat([source.subarray(0, 2), createExifWithoutOrientation(), source.subarray(2)]);
}

fs.writeFileSync(
  path.join(FIXTURES_DIR, 'benchmark-alpha.png'),
  createRgbaPng(32, 24, [32, 160, 240, 128]),
);
fs.writeFileSync(
  path.join(FIXTURES_DIR, 'benchmark-orientation-absent.jpg'),
  createOrientationAbsentJpeg(),
);

console.log('benchmark corpus fixtures created');
