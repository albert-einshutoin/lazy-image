'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');

const { ImageEngine, inspect } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

const INPUT = resolveFixture('test_100KB_1057x1057.jpg');

async function withTempDir(run) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-responsive-'));
  try {
    return await run(directory);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

async function main() {
  const engine = ImageEngine.fromPath(INPUT);
  const variants = await engine.toResponsiveSet({
    widths: [100, 50],
    format: 'webp',
  });

  assert.deepEqual(variants.map(({ width }) => width), [100, 50]);
  for (const variant of variants) {
    assert.ok(Buffer.isBuffer(variant.data));
    assert.equal(variant.bytes, variant.data.length);
    assert.equal(inspect(variant.data).format, 'webp');
    assert.equal(inspect(variant.data).width, variant.width);
    assert.equal(inspect(variant.data).height, variant.width);
  }

  const deduped = await ImageEngine.fromPath(INPUT).toResponsiveSet({
    widths: [100, 100, 50],
    format: 'webp',
  });
  assert.deepEqual(deduped.map(({ width }) => width), [100, 50]);

  await withTempDir(async (directory) => {
    const pattern = path.join(directory, 'image-{width}.webp');
    const output = await ImageEngine.fromPath(INPUT).toFilesResponsive(pattern, {
      widths: [64, 32],
      format: 'webp',
    }, '/images/image-{width}.webp');

    assert.deepEqual(output.files.map(({ width }) => width), [64, 32]);
    assert.equal(
      output.srcset,
      '/images/image-64.webp 64w, /images/image-32.webp 32w',
    );
    for (const file of output.files) {
      assert.equal(file.bytesWritten, (await fs.stat(file.path)).size);
      assert.equal(inspect(await fs.readFile(file.path)).width, file.width);
    }
  });

  assert.throws(
    () => ImageEngine.fromPath(INPUT).toResponsiveSet({ widths: [], format: 'webp' }),
    /widths must be a non-empty array of positive integers/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toResponsiveSet({ widths: [0], format: 'webp' }),
    /widths must be a non-empty array of positive integers/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toResponsiveSet({ widths: ['a'], format: 'webp' }),
    /widths must be a non-empty array of positive integers/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toResponsiveSet({ widths: [100] }),
    /format is required for responsive output/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toFilesResponsive('image.webp', { widths: [100], format: 'webp' }),
    /pattern must contain a \{width\} placeholder/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toFilesResponsive('/tmp/image-{width}.webp', { widths: [100], format: 'webp' }),
    /srcsetPattern is required when pattern is an absolute filesystem path/,
  );

  const transformed = await ImageEngine.fromPath(INPUT)
    .grayscale()
    .toResponsiveSet({ widths: [80], format: 'jpeg', quality: 80 });
  assert.equal(inspect(transformed[0].data).width, 80);

  const reusable = ImageEngine.fromPath(INPUT);
  await reusable.toResponsiveSet({ widths: [80], format: 'webp' });
  assert.equal(inspect(await reusable.toBuffer('jpeg', 80)).width, 1057);

  console.log('Responsive helper integration tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
