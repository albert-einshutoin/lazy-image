/**
 * Quality metrics (SSIM/PSNR) smoke test.
 * Verifies that identical images produce perfect metrics and
 * that lazy-image output stays close to input for a simple transform.
 */

const fs = require('fs');
const assert = require('assert');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));
const { calculateQualityMetrics } = require('../helpers/quality');

const TEST_IMAGE = resolveFixture('test_input.jpg');

async function asyncTest(name, fn) {
    try {
        await fn();
        console.log(`✅ ${name}`);
    } catch (e) {
        console.log(`❌ ${name}`);
        console.log(`   Error: ${e.message}`);
        throw e;
    }
}

async function run() {
    console.log('=== Quality Metrics Tests ===');
    const original = fs.readFileSync(TEST_IMAGE);

    await asyncTest('SSIM/PSNR are perfect for identical buffers', async () => {
        const { psnr, ssim } = await calculateQualityMetrics(original, original);
        assert(psnr === Infinity || psnr > 80, 'PSNR should be very high');
        assert(ssim > 0.99 && ssim <= 1, 'SSIM should be ~1');
    });

    await asyncTest('lazy-image encode maintains high quality', async () => {
        const output = await ImageEngine.from(original)
            .resize(800)
            .toBuffer('jpeg', 85);
        const outMeta = await require('sharp')(output).metadata();
        // Reference via sharp with same operations/dimensions
        const reference = await require('sharp')(original)
            .resize(outMeta.width, outMeta.height, { withoutEnlargement: true })
            .jpeg({ quality: 85, mozjpeg: true })
            .toBuffer();
        const referenceMatched = await require('sharp')(reference)
            .resize(outMeta.width, outMeta.height, { fit: 'fill' })
            .toBuffer();
        const { psnr, ssim } = await calculateQualityMetrics(referenceMatched, output);
        assert(psnr > 35, 'PSNR should remain high');
        assert(ssim > 0.97, 'SSIM should remain high');
    });
}

run().catch((e) => {
    console.error(e);
    process.exit(1);
});
