/**
 * Basic usage examples for lazy-image.
 *
 * Prerequisites:
 *   npm install @alberteinshutoin/lazy-image
 *   (or run from the repo root after `npm run build`)
 *
 * Run:
 *   node examples/basic.mjs
 *   node examples/basic.mjs --self-test
 */

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  fixturePath,
  hasSelfTestFlag,
  loadLazyImage,
  makeTempDir,
  removeDir,
} from './_load.mjs';

const { ImageEngine, inspectFile } = loadLazyImage();

if (hasSelfTestFlag()) {
  await runSelfTest();
} else {
  await runDemo();
}

async function runSelfTest() {
  const tmp = makeTempDir('lazy-image-basic-');

  try {
    const inputJpeg = fixturePath('test_100KB_1057x1057.jpg');
    const inputPng = fixturePath('test_100KB_1188x1188.png');

    const resizedPath = path.join(tmp, 'resized.jpg');
    const bytesWritten = await ImageEngine.fromPath(inputJpeg)
      .resize(800)
      .toFile(resizedPath, 'jpeg', 85);
    assert(bytesWritten > 0);
    assert(fs.existsSync(resizedPath));

    const webpBuffer = await ImageEngine.fromPath(inputPng)
      .resize({ width: 1200, fit: 'inside' })
      .toBuffer('webp', 80);
    assert(webpBuffer.length > 0);

    const engine = ImageEngine.fromPath(inputJpeg).resize(640);
    const [jpegBytes, webpBytes] = await Promise.all([
      engine.clone().toFile(path.join(tmp, 'hero.jpg'), 'jpeg', 85),
      engine.clone().toFile(path.join(tmp, 'hero.webp'), 'webp', 80),
    ]);
    assert(jpegBytes > 0);
    assert(webpBytes > 0);

    const meta = inspectFile(inputJpeg);
    assert(meta.width > 0);
    assert(meta.height > 0);
    assert(meta.format);

    const thumbnail = await ImageEngine.fromPath(inputJpeg).toBufferWithPreset('thumbnail');
    assert(thumbnail.length > 0);

    const processed = await ImageEngine.fromPath(inputJpeg)
      .crop(100, 100, 400, 400)
      .rotate(90)
      .grayscale()
      .toBuffer('jpeg', 80);
    assert(processed.length > 0);

    const { data, metrics } = await ImageEngine.fromPath(inputJpeg)
      .resize(600)
      .toBufferWithMetrics('webp', 80);
    assert(data.length > 0);
    assert(metrics.totalMs >= 0);
    assert(metrics.bytesOut === data.length);
  } finally {
    removeDir(tmp);
  }
}

async function runDemo() {
  // 1. Resize and save (file-to-file, recommended)
  const bytesWritten = await ImageEngine.fromPath('input.jpg')
    .resize(800)
    .toFile('output.jpg', 'jpeg', 85);

  console.log(`Wrote ${bytesWritten} bytes`);

  // 2. Convert PNG to WebP with quality control
  const webpBuffer = await ImageEngine.fromPath('photo.png')
    .resize({ width: 1200, fit: 'inside' })
    .toBuffer('webp', 80);

  console.log(`WebP buffer: ${webpBuffer.length} bytes`);

  // 3. Multiple outputs from one source (clone)
  const engine = ImageEngine.fromPath('hero.jpg').resize(1920);

  const [jpegBytes, avifBytes] = await Promise.all([
    engine.clone().toFile('hero.jpg', 'jpeg', 85),
    engine.clone().toFile('hero.avif', 'avif', 60),
  ]);

  console.log(`JPEG: ${jpegBytes} bytes, AVIF: ${avifBytes} bytes`);

  // 4. Inspect metadata without decoding
  const meta = inspectFile('input.jpg');
  console.log(`${meta.width}x${meta.height} ${meta.format}`);

  // 5. Presets for common use cases
  const thumbnail = await ImageEngine.fromPath('photo.jpg')
    .toBufferWithPreset('thumbnail');

  console.log(`Thumbnail: ${thumbnail.length} bytes`);

  // 6. Crop, rotate, and grayscale
  const processed = await ImageEngine.fromPath('photo.jpg')
    .crop(100, 100, 400, 400)
    .rotate(90)
    .grayscale()
    .toBuffer('jpeg', 80);

  console.log(`Processed: ${processed.length} bytes`);

  // 7. Encode with metrics
  const { data, metrics } = await ImageEngine.fromPath('photo.jpg')
    .resize(600)
    .toBufferWithMetrics('webp', 80);

  console.log(`Output: ${data.length} bytes`);
  console.log(`Decode: ${metrics.decodeMs}ms, Encode: ${metrics.encodeMs}ms`);
  console.log(`Compression ratio: ${metrics.compressionRatio.toFixed(2)}`);
}
