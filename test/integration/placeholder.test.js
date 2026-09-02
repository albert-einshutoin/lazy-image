'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs/promises');

const { ImageEngine, inspect } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

const INPUT = resolveFixture('test_100KB_1057x1057.jpg');

async function main() {
  const placeholder = await ImageEngine.fromPath(INPUT).toPlaceholder();
  assert.match(placeholder.dataUrl, /^data:image\/webp;base64,/);
  const webp = Buffer.from(placeholder.dataUrl.split(',', 2)[1], 'base64');
  assert.equal(inspect(webp).format, 'webp');
  assert.equal(placeholder.width, 16);
  assert.equal(placeholder.height, 16);
  assert.equal(placeholder.bytes, webp.length);
  assert.ok(placeholder.bytes < 2048);

  const landscape = await ImageEngine.from(
    await ImageEngine.fromPath(INPUT).resize({ width: 500, height: 250, fit: 'fill' }).toBuffer('jpeg', 80),
  ).toPlaceholder({ size: 32, format: 'jpeg', quality: 20 });
  assert.equal(landscape.width, 32);
  assert.equal(landscape.height, 16);
  assert.match(landscape.dataUrl, /^data:image\/jpeg;base64,/);

  const extreme = await ImageEngine.from(
    await ImageEngine.fromPath(INPUT).resize({ width: 1000, height: 1, fit: 'fill' }).toBuffer('jpeg', 80),
  ).toPlaceholder();
  assert.equal(extreme.width, 16);
  assert.equal(extreme.height, 1);

  const png = await ImageEngine.fromPath(INPUT).toPlaceholder({ format: 'png', quality: 1 });
  assert.match(png.dataUrl, /^data:image\/png;base64,/);
  assert.equal(inspect(Buffer.from(png.dataUrl.split(',', 2)[1], 'base64')).format, 'png');

  assert.throws(
    () => ImageEngine.fromPath(INPUT).toPlaceholder({ size: 3 }),
    /placeholder size must be an integer between 4 and 64/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toPlaceholder({ size: 65 }),
    /placeholder size must be an integer between 4 and 64/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toPlaceholder({ size: 16.5 }),
    /placeholder size must be an integer between 4 and 64/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toPlaceholder({ format: 'gif' }),
    /placeholder format must be 'webp', 'jpeg', or 'png'/,
  );
  assert.throws(
    () => ImageEngine.fromPath(INPUT).toPlaceholder(null),
    /placeholder options must be an object/,
  );

  const reusable = ImageEngine.fromPath(INPUT);
  await reusable.toPlaceholder();
  assert.equal(inspect(await reusable.toBuffer('jpeg', 80)).width, 1057);

  await fs.access(INPUT);
  console.log('Placeholder helper integration tests passed');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
