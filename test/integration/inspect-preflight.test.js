'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const { inspect, inspectFile } = require('../../index');
const { resolveFixture, resolveTemp } = require('../helpers/paths');
const { createApng, createGrayscalePng, createRgbaPng } = require('../helpers/png-helpers');

function assertParity(buffer, fileName) {
    const path = resolveTemp(fileName);
    fs.mkdirSync(require('node:path').dirname(path), { recursive: true });
    fs.writeFileSync(path, buffer);
    assert.deepEqual(inspectFile(path), inspect(buffer));
    fs.rmSync(path, { force: true });
}

const jpeg = fs.readFileSync(resolveFixture('test_with_exif.jpg'));
const jpegMetadata = inspect(jpeg);
assert.deepEqual(jpegMetadata, {
    width: 8,
    height: 8,
    format: 'jpeg',
    hasAlpha: false,
    isAnimated: false,
    orientation: 6,
});
assertParity(jpeg, 'inspect-preflight/oriented.jpg');

const opaquePng = createGrayscalePng(3, 2);
assert.equal(inspect(opaquePng).hasAlpha, false);
assert.equal(inspect(opaquePng).isAnimated, false);
assertParity(opaquePng, 'inspect-preflight/opaque.png');

const alphaPng = createRgbaPng(3, 2);
assert.equal(inspect(alphaPng).hasAlpha, true);
assert.equal(inspect(alphaPng).isAnimated, false);
assertParity(alphaPng, 'inspect-preflight/alpha.png');

const apng = createApng();
assert.equal(inspect(apng).hasAlpha, true);
assert.equal(inspect(apng).isAnimated, true);
assertParity(apng, 'inspect-preflight/animated.png');

assert.throws(
    () => inspect(Buffer.from('RIFF\x10\0\0\0WEBPVP8X', 'binary')),
    /E131|Failed to decode|failed to read WebP header/,
);

console.log('inspect preflight contract tests passed');
