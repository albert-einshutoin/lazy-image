/**
 * Basic tests for lazy-image
 * Run with: node test/basic.test.js
 */

const { ImageEngine, inspect, inspectFile } = require('../index');
const fs = require('fs');
const path = require('path');
const assert = require('assert');

const TEST_IMAGE = path.join(__dirname, '..', 'test_input.jpg');

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`✅ ${name}`);
        passed++;
    } catch (e) {
        console.log(`❌ ${name}`);
        console.log(`   Error: ${e.message}`);
        failed++;
    }
}

async function asyncTest(name, fn) {
    try {
        await fn();
        console.log(`✅ ${name}`);
        passed++;
    } catch (e) {
        console.log(`❌ ${name}`);
        console.log(`   Error: ${e.message}`);
        failed++;
    }
}

async function runTests() {
    console.log('=== lazy-image Basic Tests ===\n');
    
    // Sync tests
    test('inspect() returns metadata', () => {
        const buffer = fs.readFileSync(TEST_IMAGE);
        const meta = inspect(buffer);
        assert(meta.width > 0, 'width should be positive');
        assert(meta.height > 0, 'height should be positive');
        assert(meta.format === 'jpeg', 'format should be jpeg');
    });

    test('inspectFile() returns metadata', () => {
        const meta = inspectFile(TEST_IMAGE);
        assert(meta.width > 0, 'width should be positive');
        assert(meta.height > 0, 'height should be positive');
    });

    // Async tests
    const buffer = fs.readFileSync(TEST_IMAGE);

    await asyncTest('basic JPEG encoding works', async () => {
        const result = await ImageEngine.from(buffer).toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('resize works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
        // Note: For very small images (1x1), resizing may increase file size
    });

    await asyncTest('WebP encoding works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('webp', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('AVIF encoding works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('avif', 60);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('PNG encoding works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('png');
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('rotate(90) works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).rotate(90).toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('grayscale() works', async () => {
        const result = await ImageEngine.from(buffer).resize(100).grayscale().toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('chain multiple operations', async () => {
        const result = await ImageEngine.from(buffer)
            .resize(200)
            .rotate(180)
            .grayscale()
            .toBuffer('jpeg', 75);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('clone() allows multi-output', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        const [jpeg, webp] = await Promise.all([
            engine.clone().toBuffer('jpeg', 80),
            engine.clone().toBuffer('webp', 80),
        ]);
        assert(jpeg.length > 0, 'JPEG should have content');
        assert(webp.length > 0, 'WebP should have content');
    });

    await asyncTest('fromPath() works', async () => {
        const result = await ImageEngine.fromPath(TEST_IMAGE)
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('toFile() works', async () => {
        const outPath = path.join(__dirname, 'test_output.jpg');
        const bytes = await ImageEngine.fromPath(TEST_IMAGE)
            .resize(100)
            .toFile(outPath, 'jpeg', 80);
        assert(bytes > 0, 'bytes written should be positive');
        assert(fs.existsSync(outPath), 'file should exist');
        fs.unlinkSync(outPath);
    });

    // Error handling tests
    await asyncTest('invalid rotation angle throws error', async () => {
        let threw = false;
        try {
            await ImageEngine.from(buffer).rotate(45).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            assert(e.message.includes('unsupported rotation angle'), 'error message should mention rotation');
        }
        assert(threw, 'should have thrown an error');
    });

    await asyncTest('invalid crop bounds throws error', async () => {
        let threw = false;
        try {
            await ImageEngine.from(buffer).crop(10000, 10000, 1000, 1000).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            assert(e.message.includes('crop bounds') || e.message.includes('exceed'), 'error should mention bounds');
        }
        assert(threw, 'should have thrown an error');
    });

    await asyncTest('invalid format throws error', async () => {
        let threw = false;
        try {
            await ImageEngine.from(buffer).toBuffer('invalid_format', 80);
        } catch (e) {
            threw = true;
        }
        assert(threw, 'should have thrown an error');
    });

    // Preset tests
    await asyncTest('preset("thumbnail") works', async () => {
        const engine = ImageEngine.from(buffer);
        const preset = engine.preset('thumbnail');
        assert(preset.format === 'webp', 'thumbnail format should be webp');
        assert(preset.quality === 75, 'thumbnail quality should be 75');
        assert(preset.width === 150, 'thumbnail width should be 150');
        assert(preset.height === 150, 'thumbnail height should be 150');
        const result = await engine.toBuffer(preset.format, preset.quality);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('preset("avatar") works', async () => {
        const engine = ImageEngine.from(buffer);
        const preset = engine.preset('avatar');
        assert(preset.format === 'webp', 'avatar format should be webp');
        assert(preset.quality === 80, 'avatar quality should be 80');
        assert(preset.width === 200, 'avatar width should be 200');
    });

    await asyncTest('preset("hero") works', async () => {
        const engine = ImageEngine.from(buffer);
        const preset = engine.preset('hero');
        assert(preset.format === 'jpeg', 'hero format should be jpeg');
        assert(preset.quality === 85, 'hero quality should be 85');
        assert(preset.width === 1920, 'hero width should be 1920');
    });

    await asyncTest('preset("social") works', async () => {
        const engine = ImageEngine.from(buffer);
        const preset = engine.preset('social');
        assert(preset.format === 'jpeg', 'social format should be jpeg');
        assert(preset.width === 1200, 'social width should be 1200');
        assert(preset.height === 630, 'social height should be 630');
    });

    await asyncTest('invalid preset throws error', async () => {
        let threw = false;
        try {
            ImageEngine.from(buffer).preset('invalid_preset');
        } catch (e) {
            threw = true;
            assert(e.message.includes('unknown preset'), 'error should mention unknown preset');
        }
        assert(threw, 'should have thrown an error');
    });

    // Default quality tests (v0.7.2+)
    await asyncTest('JPEG default quality is 85', async () => {
        // Test that JPEG uses quality 85 when not specified
        // We can't directly test the quality value, but we can verify it works
        const result1 = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg');
        const result2 = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg', 85);
        // Both should produce valid output
        assert(result1.length > 0, 'default quality should work');
        assert(result2.length > 0, 'explicit quality 85 should work');
        // Default quality (85) should produce similar or larger file than explicit 85
        // (they should be the same since default is 85)
        assert(Math.abs(result1.length - result2.length) < result1.length * 0.1, 
            'default quality should match explicit 85');
    });

    await asyncTest('WebP default quality is 80', async () => {
        const result1 = await ImageEngine.from(buffer).resize(100).toBuffer('webp');
        const result2 = await ImageEngine.from(buffer).resize(100).toBuffer('webp', 80);
        assert(result1.length > 0, 'default quality should work');
        assert(result2.length > 0, 'explicit quality 80 should work');
        assert(Math.abs(result1.length - result2.length) < result1.length * 0.1,
            'default quality should match explicit 80');
    });

    await asyncTest('AVIF default quality is 60', async () => {
        const result1 = await ImageEngine.from(buffer).resize(100).toBuffer('avif');
        const result2 = await ImageEngine.from(buffer).resize(100).toBuffer('avif', 60);
        assert(result1.length > 0, 'default quality should work');
        assert(result2.length > 0, 'explicit quality 60 should work');
        assert(Math.abs(result1.length - result2.length) < result1.length * 0.1,
            'default quality should match explicit 60');
    });

    // Batch processing concurrency test (v0.7.3+)
    await asyncTest('processBatch with concurrency control works', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        const testDir = path.join(__dirname, 'test_batch_output');
        try {
            // Test with custom concurrency (2 workers)
            const results = await engine.processBatch(
                [TEST_IMAGE, TEST_IMAGE],
                testDir,
                'jpeg',
                80,
                2  // concurrency
            );
            assert(results.length === 2, 'should process 2 images');
            assert(results.every(r => r.success), 'all should succeed');
        } finally {
            // Cleanup
            if (fs.existsSync(testDir)) {
                fs.readdirSync(testDir).forEach(file => {
                    fs.unlinkSync(path.join(testDir, file));
                });
                fs.rmdirSync(testDir);
            }
        }
    });

    // Summary
    console.log(`\n=== Results: ${passed} passed, ${failed} failed ===`);
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(e => {
    console.error('Test runner error:', e);
    process.exit(1);
});

