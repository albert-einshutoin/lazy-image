/**
 * Basic tests for lazy-image
 * Run with: node test/basic.test.js
 */

const fs = require('fs');
const path = require('path');
const assert = require('assert');
const { resolveRoot, resolveFixture, resolveTemp } = require('../helpers/paths');
const { ImageEngine, ErrorCategory, getErrorCategory, inspect, inspectFile } = require(resolveRoot('index'));

const TEST_IMAGE = resolveFixture('test_input.jpg');

function assertCategory(category, expected, message) {
    assert.notStrictEqual(category, null, 'error.category should be set');
    assert.strictEqual(category, expected, message);
}

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

    await asyncTest('toBufferWithMetrics returns metrics', async () => {
        const { data, metrics } = await ImageEngine.from(buffer)
            .resize(200)
            .toBufferWithMetrics('jpeg', 80);
        
        assert(data.length > 0, 'output should have content');
        assert(typeof metrics === 'object', 'metrics should be an object');
        assert.strictEqual(metrics.version, '1.0.0', 'version should be set');

        // New productized fields
        assert(typeof metrics.decodeMs === 'number', 'decodeMs should be a number');
        assert(metrics.decodeMs >= 0, 'decodeMs should be non-negative');
        assert(typeof metrics.opsMs === 'number', 'opsMs should be a number');
        assert(metrics.opsMs >= 0, 'opsMs should be non-negative');
        assert(typeof metrics.encodeMs === 'number', 'encodeMs should be a number');
        assert(metrics.encodeMs >= 0, 'encodeMs should be non-negative');
        assert(typeof metrics.totalMs === 'number', 'totalMs should be a number');
        assert(metrics.totalMs >= 0, 'totalMs should be non-negative');
        assert(typeof metrics.peakRss === 'number', 'peakRss should be a number');
        assert(metrics.peakRss >= 0, 'peakRss should be non-negative');
        assert(typeof metrics.bytesIn === 'number', 'bytesIn should be a number');
        assert(metrics.bytesIn > 0, 'bytesIn should be positive');
        assert(typeof metrics.bytesOut === 'number', 'bytesOut should be a number');
        assert(metrics.bytesOut > 0, 'bytesOut should be positive');
        assert(Array.isArray(metrics.policyViolations), 'policyViolations should be an array');
        assert(typeof metrics.metadataStripped === 'boolean', 'metadataStripped should be boolean');
        assert(typeof metrics.iccPreserved === 'boolean', 'iccPreserved should be boolean');
        assert(typeof metrics.formatOut === 'string', 'formatOut should be string');
        assert(metrics.formatOut === 'jpeg', 'formatOut should match requested output');
        // formatIn may be null if format detection fails, or a string if detected
        assert(metrics.formatIn === null || typeof metrics.formatIn === 'string', 'formatIn should be null or string');
        if (metrics.formatIn !== null) {
            assert(metrics.formatIn.length > 0, 'formatIn should not be empty string if not null');
        }

        // Legacy aliases remain stable
        assert.strictEqual(metrics.decodeTime, metrics.decodeMs, 'decodeTime mirrors decodeMs');
        assert.strictEqual(metrics.processTime, metrics.opsMs, 'processTime mirrors opsMs');
        assert.strictEqual(metrics.encodeTime, metrics.encodeMs, 'encodeTime mirrors encodeMs');
        assert.strictEqual(metrics.memoryPeak, metrics.peakRss, 'memoryPeak mirrors peakRss');
        assert.strictEqual(metrics.inputSize, metrics.bytesIn, 'inputSize mirrors bytesIn');
        assert.strictEqual(metrics.outputSize, metrics.bytesOut, 'outputSize mirrors bytesOut');

        assert(typeof metrics.cpuTime === 'number', 'cpuTime should be a number');
        assert(metrics.cpuTime >= 0, 'cpuTime should be non-negative');
        assert(typeof metrics.processingTime === 'number', 'processingTime should be a number');
        assert(metrics.processingTime >= 0, 'processingTime should be non-negative');
        assert(typeof metrics.compressionRatio === 'number', 'compressionRatio should be a number');
        assert(metrics.compressionRatio >= 0, 'compressionRatio should be non-negative');
        assert(metrics.compressionRatio <= 1 || metrics.bytesOut > metrics.bytesIn, 
            'compressionRatio should be <= 1 or output larger than input');
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

    await asyncTest('toBuffer() is non-destructive (multiple calls without clone)', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        // First call
        const jpeg = await engine.toBuffer('jpeg', 80);
        assert(jpeg.length > 0, 'JPEG should have content');
        // Second call on the same instance (should work without clone)
        const webp = await engine.toBuffer('webp', 80);
        assert(webp.length > 0, 'WebP should have content');
        // Third call
        const png = await engine.toBuffer('png');
        assert(png.length > 0, 'PNG should have content');
    });

    await asyncTest('toBufferWithMetrics() is non-destructive', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        // First call
        const result1 = await engine.toBufferWithMetrics('jpeg', 80);
        assert(result1.data.length > 0, 'First JPEG should have content');
        assert(result1.metrics.decodeMs > 0, 'Metrics should include decode time');
        // Second call on the same instance
        const result2 = await engine.toBufferWithMetrics('webp', 80);
        assert(result2.data.length > 0, 'Second WebP should have content');
        assert(result2.metrics.decodeMs > 0, 'Metrics should include decode time');
    });

    await asyncTest('toFile() is non-destructive', async () => {
        const engine = ImageEngine.fromPath(TEST_IMAGE).resize(100);
        const outPath1 = resolveTemp('test_output1.jpg');
        const outPath2 = resolveTemp('test_output2.webp');
        try {
            // First call
            const bytes1 = await engine.toFile(outPath1, 'jpeg', 80);
            assert(bytes1 > 0, 'First file should be written');
            assert(fs.existsSync(outPath1), 'First file should exist');
            // Second call on the same instance
            const bytes2 = await engine.toFile(outPath2, 'webp', 80);
            assert(bytes2 > 0, 'Second file should be written');
            assert(fs.existsSync(outPath2), 'Second file should exist');
        } finally {
            if (fs.existsSync(outPath1)) fs.unlinkSync(outPath1);
            if (fs.existsSync(outPath2)) fs.unlinkSync(outPath2);
        }
    });

    await asyncTest('fromPath() works', async () => {
        const result = await ImageEngine.fromPath(TEST_IMAGE)
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('toFile() works', async () => {
        const outPath = resolveTemp('test_output.jpg');
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
            assert(e.message.includes('rotation') || e.message.includes('angle'), 'error message should mention rotation');
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

    await asyncTest('resize validates dimensions and preserves error category/code', async () => {
        let threw = false;
        try {
            // width = 0 is invalid; should throw synchronously
            ImageEngine.from(buffer).resize(0);
        } catch (e) {
            threw = true;
            assert.strictEqual(e.code, 'LAZY_IMAGE_USER_ERROR', 'error.code should be UserError');
            assert.strictEqual(e.errorCode, 'E203', 'fine-grained errorCode should be set');
            assert(typeof e.recoveryHint === 'string' && e.recoveryHint.length > 0, 'recoveryHint should be present');
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.UserError, 'resize validation should be UserError');
            assert(
                e.message.includes('width') && e.message.includes('must be between'),
                `message should mention width bounds: ${e.message}`
            );
        }
        assert(threw, 'should have thrown for invalid resize width');
    });

    await asyncTest('brightness validates range and preserves error category/code', async () => {
        let threw = false;
        try {
            ImageEngine.from(buffer).brightness(200);
        } catch (e) {
            threw = true;
            assert.strictEqual(e.code, 'LAZY_IMAGE_USER_ERROR', 'error.code should be UserError');
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.UserError, 'brightness validation should be UserError');
            assert(
                e.message.includes('brightness') && e.message.includes('-100') && e.message.includes('100'),
                `message should mention brightness range: ${e.message}`
            );
        }
        assert(threw, 'should have thrown for invalid brightness');
    });

    // Error category tests
    // Note: These tests check for error.code property, which is set by create_napi_error_with_code()
    // Currently, not all error sites use this function, so some tests may fail until all error sites are updated.
    await asyncTest('error category: UserError for invalid rotation', async () => {
        try {
            await ImageEngine.from(buffer).rotate(45).toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.UserError, 'invalid rotation should be UserError');
            assert(e.message, 'error should have message field');
            // Error message should NOT have prefix (backward compatibility)
            // Check for both possible prefix formats
            assert(!e.message.startsWith('LAZY_IMAGE_USER_ERROR:UserError:'), 'message should NOT have LAZY_IMAGE_USER_ERROR:UserError: prefix');
            assert(!e.message.startsWith('UserError:'), 'message should NOT have UserError: prefix');
        }
    });

    await asyncTest('error category: UserError for invalid crop bounds', async () => {
        try {
            await ImageEngine.from(buffer).crop(10000, 10000, 1000, 1000).toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.UserError, 'invalid crop bounds should be UserError');
            // Error message should NOT have prefix
            assert(!e.message.startsWith('LAZY_IMAGE_USER_ERROR:UserError:'), 'message should NOT have LAZY_IMAGE_USER_ERROR:UserError: prefix');
            assert(!e.message.startsWith('UserError:'), 'message should NOT have UserError: prefix');
        }
    });

    await asyncTest('error category: CodecError for invalid format', async () => {
        try {
            await ImageEngine.from(buffer).toBuffer('invalid_format', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.CodecError, 'invalid format should be CodecError');
            // Error message should NOT have prefix
            assert(!e.message.startsWith('LAZY_IMAGE_CODEC_ERROR:CodecError:'), 'message should NOT have LAZY_IMAGE_CODEC_ERROR:CodecError: prefix');
            assert(!e.message.startsWith('CodecError:'), 'message should NOT have CodecError: prefix');
        }
    });

    await asyncTest('error category: UserError for file not found', async () => {
        try {
            await ImageEngine.fromPath('/nonexistent/file.jpg').toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            const category = getErrorCategory(e);
            assertCategory(category, ErrorCategory.UserError, 'file not found should be UserError');
        }
    });

    await asyncTest('getErrorCategory returns null for non-lazy-image errors', async () => {
        const regularError = new Error('Regular error');
        const category = getErrorCategory(regularError);
        assert.strictEqual(category, null, 'non-lazy-image errors should return null');
    });

    await asyncTest('getErrorCategory handles null/undefined', async () => {
        assert.strictEqual(getErrorCategory(null), null);
        assert.strictEqual(getErrorCategory(undefined), null);
    });

    await asyncTest('error category: ResourceLimit for file write error', async () => {
        // More reliable test: specify existing file as directory (guaranteed to error)
        // This ensures an error by trying to create a file under an existing file
        let threw = false;
        let category = null;
        try {
            // Specify existing file as directory
            // This will fail because we're trying to create a file under a file
            const invalidPath = path.join(TEST_IMAGE, 'output.jpg');
            await ImageEngine.from(buffer)
                .resize(100, 100)
                .toFile(invalidPath, 'jpeg', 80);
        } catch (e) {
            threw = true;
            category = getErrorCategory(e);
            // Verify error category is ResourceLimit
            if (category !== null) {
                assertCategory(
                    category, 
                    ErrorCategory.ResourceLimit, 
                    'file write error should be ResourceLimit'
                );
            }
        }
        
        // Verify that error was thrown (required)
        assert(threw, 'should throw error when trying to write to invalid path');
        assert(category === ErrorCategory.ResourceLimit, 'error category should be ResourceLimit');
    });

    await asyncTest('processBatch error results expose category', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        const tempDir = resolveTemp('batch_error_meta');
        if (!fs.existsSync(tempDir)) {
            fs.mkdirSync(tempDir, { recursive: true });
        }
        const invalidPath = '/nonexistent/batch-input.jpg';
        const results = await engine.processBatch([invalidPath], tempDir, 'jpeg', 80, undefined, 1);
        assert.strictEqual(results.length, 1, 'should return one result');
        const result = results[0];
        assert.strictEqual(result.success, false, 'result should be marked as failure');
        assert.strictEqual(result.errorCode, 'E100', 'errorCode should carry fine-grained code');
        assert.strictEqual(result.errorCategory, ErrorCategory.UserError, 'error category should be UserError');
        assert(result.error && result.error.includes(invalidPath), 'error message should include source path');
    });

    // Note: Testing InternalBug category is difficult because it requires triggering
    // actual internal errors, which should only happen due to implementation bugs.
    // In normal usage, InternalBug errors should not occur.

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
            assert(e.message.includes('preset') || e.message.includes('unknown'), 'error should mention unknown preset');
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
        const testDir = resolveTemp('test_batch_output');
        try {
            // Test with custom concurrency (2 workers)
            const results = await engine.processBatch(
                [TEST_IMAGE, TEST_IMAGE],
                testDir,
                'jpeg',
                80,
                undefined,  // fastMode (optional)
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

    // Thread pool coordination test (v0.7.8+)
    await asyncTest('processBatch with default concurrency (auto-calculated) works', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        const testDir = resolveTemp('test_batch_auto');
        try {
            // Test with concurrency=0 (default, auto-calculated)
            // This should automatically balance threads using available_parallelism()
            const results = await engine.processBatch(
                [TEST_IMAGE, TEST_IMAGE],
                testDir,
                'jpeg',
                80,
                undefined,  // fastMode (optional)
                0  // auto-detect
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
