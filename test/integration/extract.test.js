/**
 * Verify resize+crop fusion is exposed through JS API.
 * Checks that fused path produces correct output and lowers peak RSS
 * compared to an explicit two-step pipeline.
 */

const assert = require('assert');
const { resolveRoot, resolveFixture } = require('../helpers/paths');
const { ImageEngine, inspect } = require(resolveRoot('index'));

const SOURCE = resolveFixture('test_4.5MB_5000x5000.png');
const RESIZE_W = 2000;
const RESIZE_H = 2000;
const CROP = { x: 100, y: 120, width: 800, height: 600 };

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        const result = fn();
        if (result instanceof Promise) {
            throw new Error('Use asyncTest for async functions');
        }
        console.log(`✅ ${name}`);
        passed++;
    } catch (e) {
        console.error(`❌ ${name}`);
        console.error(`   ${e.message}`);
        failed++;
    }
}

async function asyncTest(name, fn) {
    try {
        await fn();
        console.log(`✅ ${name}`);
        passed++;
    } catch (e) {
        console.error(`❌ ${name}`);
        console.error(`   ${e.message}`);
        failed++;
    }
}

async function run() {
    console.log('=== resize+crop fusion (Extract) integration tests ===');

    await asyncTest('fused resize+crop yields expected dimensions', async () => {
        const { data } = await ImageEngine.fromPath(SOURCE)
            .resize(RESIZE_W, RESIZE_H)
            .crop(CROP.x, CROP.y, CROP.width, CROP.height)
            .toBufferWithMetrics('png');

        assert(data.length > 0, 'output buffer should not be empty');

        const meta = inspect(data);
        assert.strictEqual(meta.width, CROP.width, 'width should match crop width');
        assert.strictEqual(meta.height, CROP.height, 'height should match crop height');
    });

    await asyncTest('fused path uses less or equal peak RSS than two-step pipeline', async () => {
        // Fused path (Extract)
        const fused = await ImageEngine.fromPath(SOURCE)
            .resize(RESIZE_W, RESIZE_H)
            .crop(CROP.x, CROP.y, CROP.width, CROP.height)
            .toBufferWithMetrics('png');

        // Two-step: resize then new pipeline for crop (forces extra buffer)
        const resizedOnly = await ImageEngine.fromPath(SOURCE)
            .resize(RESIZE_W, RESIZE_H)
            .toBufferWithMetrics('png');

        const cropped = await ImageEngine.from(resizedOnly.data)
            .crop(CROP.x, CROP.y, CROP.width, CROP.height)
            .toBufferWithMetrics('png');

        const fusedPeak = fused.metrics.peakRss;
        const twoStepPeak = Math.max(resizedOnly.metrics.peakRss, cropped.metrics.peakRss);

        assert(
            fusedPeak <= twoStepPeak,
            `fused peakRss (${fusedPeak}) should be <= two-step peakRss (${twoStepPeak})`
        );
    });

    if (failed > 0) {
        console.error(`\n❌ ${failed} test(s) failed`);
        process.exit(1);
    }
    console.log(`\n✅ All ${passed} test(s) passed`);
}

run();
