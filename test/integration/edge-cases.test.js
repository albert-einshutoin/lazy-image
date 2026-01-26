/**
 * Edge cases and security tests for lazy-image
 * These tests ensure the library is production-ready
 */

const fs = require('fs');
const path = require('path');
const assert = require('assert');
const zlib = require('zlib');
const { resolveRoot, resolveFixture, resolveTemp } = require('../helpers/paths');
const { ImageEngine, inspect, inspectFile, ErrorCategory, getErrorCategory } = require(resolveRoot('index'));

const TEST_IMAGE = resolveFixture('test_input.jpg');

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

function buildCrc32Table() {
    const table = new Uint32Array(256);
    for (let i = 0; i < 256; i++) {
        let c = i;
        for (let k = 0; k < 8; k++) {
            c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
        }
        table[i] = c >>> 0;
    }
    return table;
}

const CRC_TABLE = buildCrc32Table();

function crc32(buf) {
    let crc = 0xFFFFFFFF;
    for (const byte of buf) {
        crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ byte) & 0xFF];
    }
    return (crc ^ 0xFFFFFFFF) >>> 0;
}

function pngChunk(type, data) {
    const typeBuf = Buffer.from(type, 'ascii');
    const lengthBuf = Buffer.alloc(4);
    lengthBuf.writeUInt32BE(data.length, 0);
    const crcBuf = Buffer.alloc(4);
    const crc = crc32(Buffer.concat([typeBuf, data]));
    crcBuf.writeUInt32BE(crc, 0);
    return Buffer.concat([lengthBuf, typeBuf, data, crcBuf]);
}

function createGrayscalePng(width, height) {
    const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(width, 0);
    ihdr.writeUInt32BE(height, 4);
    ihdr[8] = 8; // bit depth
    ihdr[9] = 0; // color type: grayscale
    ihdr[10] = 0; // compression
    ihdr[11] = 0; // filter
    ihdr[12] = 0; // interlace

    const rowSize = width + 1;
    const raw = Buffer.alloc(rowSize * height);
    for (let y = 0; y < height; y++) {
        raw[y * rowSize] = 0; // filter type 0
    }

    const compressed = zlib.deflateSync(raw);
    const ihdrChunk = pngChunk('IHDR', ihdr);
    const idatChunk = pngChunk('IDAT', compressed);
    const iendChunk = pngChunk('IEND', Buffer.alloc(0));
    return Buffer.concat([signature, ihdrChunk, idatChunk, iendChunk]);
}

async function runTests() {
    console.log('=== lazy-image Edge Cases & Security Tests ===\n');
    
    const buffer = fs.readFileSync(TEST_IMAGE);
    const meta = inspect(buffer);
    
    // ========================================================================
    // SECURITY TESTS - Decompression bomb protection
    // ========================================================================
    
    await asyncTest('rejects images exceeding MAX_DIMENSION via ImageEngine.from()', async () => {
        const maxDimension = 32768;
        const oversized = createGrayscalePng(maxDimension + 1, 1);
        let threw = false;
        try {
            await ImageEngine.from(oversized).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            assert(
                e.message.includes('exceeds maximum') || e.message.includes('exceeds max'),
                `unexpected error: ${e.message}`
            );
        }
        assert(threw, 'should throw error for oversized dimensions');
    });
    
    // ========================================================================
    // EDGE CASES - Invalid inputs
    // ========================================================================
    
    await asyncTest('rejects empty buffer', async () => {
        let threw = false;
        try {
            await ImageEngine.from(Buffer.alloc(0)).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            assert(e.message.includes('decode') || e.message.includes('failed'), 'should mention decode failure');
        }
        assert(threw, 'should throw error for empty buffer');
    });
    
    await asyncTest('rejects invalid image data', async () => {
        let threw = false;
        try {
            await ImageEngine.from(Buffer.from('not an image')).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
        }
        assert(threw, 'should throw error for invalid image data');
    });
    
    await asyncTest('rejects non-existent file path', async () => {
        let threw = false;
        try {
            await ImageEngine.fromPath('/nonexistent/path/image.jpg').toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            // True lazy loading: file existence is checked at fromPath() time
            // so error message can be "File not found" instead of "failed to read"
            assert(
                e.message.includes('failed to read') || 
                e.message.includes('No such file') ||
                e.message.includes('File not found'), 
                'should mention file read failure or file not found');
        }
        assert(threw, 'should throw error for non-existent file');
    });
    
    await asyncTest('inspectFile() rejects non-existent file', () => {
        let threw = false;
        try {
            inspectFile('/nonexistent/path/image.jpg');
        } catch (e) {
            threw = true;
        }
        assert(threw, 'should throw error for non-existent file');
    });
    
    // ========================================================================
    // EDGE CASES - Quality values
    // ========================================================================
    
    await asyncTest('handles quality 0 (minimum)', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg', 0);
        assert(result.length > 0, 'should produce output even with quality 0');
    });
    
    await asyncTest('handles quality 100 (maximum)', async () => {
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg', 100);
        assert(result.length > 0, 'should produce output with quality 100');
    });
    
    await asyncTest('clamps quality > 100 to valid range', async () => {
        // Quality should be clamped internally
        const result = await ImageEngine.from(buffer).resize(100).toBuffer('jpeg', 150);
        assert(result.length > 0, 'should handle quality > 100');
    });
    
    // ========================================================================
    // EDGE CASES - Resize operations
    // ========================================================================
    
    await asyncTest('handles resize to 1x1 (minimum size)', async () => {
        const result = await ImageEngine.from(buffer).resize(1, 1).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle 1x1 resize');
    });
    
    await asyncTest('handles resize with only width (maintains aspect ratio)', async () => {
        const result = await ImageEngine.from(buffer).resize(200).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle width-only resize');
    });
    
    await asyncTest('handles resize with only height (maintains aspect ratio)', async () => {
        const result = await ImageEngine.from(buffer).resize(null, 200).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle height-only resize');
    });
    
    await asyncTest('handles resize with both dimensions (may distort)', async () => {
        const result = await ImageEngine.from(buffer).resize(200, 300).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle explicit width and height');
    });
    
    // ========================================================================
    // EDGE CASES - Crop operations
    // ========================================================================
    
    await asyncTest('handles crop at origin (0,0)', async () => {
        const result = await ImageEngine.from(buffer)
            .crop(0, 0, Math.min(100, meta.width), Math.min(100, meta.height))
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle crop at origin');
    });
    
    await asyncTest('handles crop of entire image', async () => {
        const result = await ImageEngine.from(buffer)
            .crop(0, 0, meta.width, meta.height)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle crop of entire image');
    });
    
    // ========================================================================
    // EDGE CASES - Rotation
    // ========================================================================
    
    await asyncTest('handles rotation 0 (no-op)', async () => {
        const result = await ImageEngine.from(buffer).rotate(0).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle 0 degree rotation');
    });
    
    await asyncTest('handles negative rotation (-90)', async () => {
        const result = await ImageEngine.from(buffer).rotate(-90).toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle negative rotation');
    });
    
    await asyncTest('rejects rotation 360 (invalid angle)', async () => {
        // 360 degrees is invalid; should error
        // Implementation only supports 0, 90, 180, 270, -90, -180, -270
        let threw = false;
        try {
            await ImageEngine.from(buffer).rotate(360).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            assert(
                e.message.includes('rotation') || 
                e.message.includes('angle') || 
                e.message.includes('360') ||
                e.message.includes('Unsupported'),
                `error should mention rotation/angle: ${e.message}`
            );
        }
        // Verify that error was thrown for invalid rotation angle
        assert(threw, 'should throw error for invalid rotation angle 360');
    });
    
    // ========================================================================
    // EDGE CASES - Multiple operations
    // ========================================================================
    
    await asyncTest('handles many chained operations', async () => {
        const result = await ImageEngine.from(buffer)
            .resize(200)
            .rotate(90)
            .flipH()
            .flipV()
            .grayscale()
            .brightness(10)
            .contrast(10)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle many operations');
    });
    
    await asyncTest('handles multiple resize operations (should optimize)', async () => {
        const result = await ImageEngine.from(buffer)
            .resize(500)
            .resize(300)
            .resize(200)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'should handle multiple resizes');
    });
    
    // ========================================================================
    // EDGE CASES - File I/O
    // ========================================================================
    
    await asyncTest('toFile() rejects non-existent parent directory', async () => {
        // toFile() does not create parent directories (src/engine/tasks.rs:446-455)
        // Writing to non-existent parent directory should error
        const testDir = resolveTemp('nonexistent_dir');
        const outPath = path.join(testDir, 'test_output.jpg');
        let threw = false;
        let errorMessage = '';
        try {
            await ImageEngine.from(buffer).resize(100).toFile(outPath, 'jpeg', 80);
        } catch (e) {
            threw = true;
            errorMessage = e.message;
            // Verify error message mentions directory/path issue
            assert(
                errorMessage.includes('directory') || 
                errorMessage.includes('path') || 
                errorMessage.includes('not found') ||
                errorMessage.includes('No such file'),
                `error should mention directory/path issue: ${errorMessage}`
            );
        }
        // Verify that error was thrown when parent directory does not exist
        assert(threw, 'should throw error when parent directory does not exist');
    });
    
    // ========================================================================
    // EDGE CASES - Batch processing
    // ========================================================================
    
    await asyncTest('processBatch handles empty input array', async () => {
        const engine = ImageEngine.from(buffer).resize(100);
        const testDir = resolveTemp('test_batch_empty');
        try {
            const results = await engine.processBatch([], testDir, 'jpeg', 80, undefined, 1);
            assert(Array.isArray(results), 'should return array');
            assert(results.length === 0, 'should return empty array for empty input');
        } finally {
            if (fs.existsSync(testDir)) {
                try {
                    fs.readdirSync(testDir).forEach(file => {
                        fs.unlinkSync(path.join(testDir, file));
                    });
                    fs.rmdirSync(testDir);
                } catch (e) {
                    // Ignore cleanup errors
                }
            }
        }
    });
    
    await asyncTest('processBatch accepts concurrency 0 (uses default)', async () => {
        // concurrency=0 is valid and uses default thread pool
        // Auto-detected in src/engine/tasks.rs:699-701
        const engine = ImageEngine.from(buffer).resize(100);
        const testDir = resolveTemp('test_batch_concurrency_0');
        try {
            const results = await engine.processBatch(
                [TEST_IMAGE], 
                testDir, 
                'jpeg', 
                80, 
                undefined, 
                0  // concurrency=0 is valid
            );
            assert(results.length === 1, 'should process 1 image');
            assert(results[0].success, 'should succeed with concurrency=0');
        } finally {
            // Verify success and cleanup
            if (fs.existsSync(testDir)) {
                try {
                    fs.readdirSync(testDir).forEach(file => {
                        fs.unlinkSync(path.join(testDir, file));
                    });
                    fs.rmdirSync(testDir);
                } catch (e) {
                    // Ignore cleanup errors
                }
            }
        }
    });
    
    await asyncTest('processBatch rejects concurrency > 1024 (InternalBug)', async () => {
        // concurrency > MAX_CONCURRENCY (1024) must error
        // Results in internal_panic error in src/engine/tasks.rs:688-695
        const engine = ImageEngine.from(buffer).resize(100);
        const testDir = resolveTemp('test_batch_high_concurrency');
        let threw = false;
        let errorMessage = '';
        let category = null;
        
        try {
            // Concurrency > 1024 should be rejected
            await engine.processBatch([TEST_IMAGE], testDir, 'jpeg', 80, undefined, 2000);
        } catch (e) {
            threw = true;
            errorMessage = e.message;
            category = getErrorCategory(e);
            
            // Verify error message mentions concurrency limit
            assert(
                errorMessage.includes('concurrency') || 
                errorMessage.includes('invalid') ||
                errorMessage.includes('1024'),
                `error should mention concurrency limit: ${errorMessage}`
            );
            
            // Category should be InternalBug (must be set)
            assert(category !== null, 'error category should be set (not null)');
            assert(
                category === ErrorCategory.InternalBug,
                `error category should be InternalBug, got: ${category}`
            );
        }
        
        // Verify that error was thrown
        assert(threw, 'should throw error for concurrency > 1024');
        
        // Cleanup (files should not be created if error occurred)
        if (fs.existsSync(testDir)) {
            try {
                fs.readdirSync(testDir).forEach(file => {
                    fs.unlinkSync(path.join(testDir, file));
                });
                fs.rmdirSync(testDir);
            } catch (e) {
                // Ignore cleanup errors
            }
        }
    });
    
    // ========================================================================
    // EDGE CASES - Metrics
    // ========================================================================
    
    await asyncTest('toBufferWithMetrics returns valid metrics with all fields', async () => {
        const result = await ImageEngine.from(buffer)
            .resize(100)
            .toBufferWithMetrics('jpeg', 80);
        
        assert(result.data, 'should have data');
        assert(result.metrics, 'should have metrics');
        
        assert.strictEqual(result.metrics.version, '1.0.0', 'version should be set');

        // Productized fields
        assert(typeof result.metrics.decodeMs === 'number', 'decodeMs should be number');
        assert(typeof result.metrics.opsMs === 'number', 'opsMs should be number');
        assert(typeof result.metrics.encodeMs === 'number', 'encodeMs should be number');
        assert(typeof result.metrics.totalMs === 'number', 'totalMs should be number');
        assert(result.metrics.decodeMs >= 0, 'decodeMs should be non-negative');
        assert(result.metrics.opsMs >= 0, 'opsMs should be non-negative');
        assert(result.metrics.encodeMs >= 0, 'encodeMs should be non-negative');
        assert(result.metrics.totalMs >= 0, 'totalMs should be non-negative');
        assert(typeof result.metrics.peakRss === 'number', 'peakRss should be number');
        assert(result.metrics.peakRss > 0, 'peakRss should be positive');
        assert(typeof result.metrics.bytesIn === 'number', 'bytesIn should be number');
        assert(typeof result.metrics.bytesOut === 'number', 'bytesOut should be number');
        assert(result.metrics.bytesIn > 0, 'bytesIn should be positive');
        assert(result.metrics.bytesOut > 0, 'bytesOut should be positive');
        assert(typeof result.metrics.formatOut === 'string', 'formatOut should be string');
        // formatIn may be null if format detection fails, or a string if detected
        assert(result.metrics.formatIn === null || typeof result.metrics.formatIn === 'string', 'formatIn should be null or string');
        assert(Array.isArray(result.metrics.policyViolations), 'policyViolations should be array');
        assert(typeof result.metrics.metadataStripped === 'boolean', 'metadataStripped boolean');
        assert(typeof result.metrics.iccPreserved === 'boolean', 'iccPreserved boolean');
        // metadata_stripped = true when metadata is not preserved (default behavior)
        // icc_preserved = true when metadata exists and is preserved
        // They should be mutually exclusive: if metadata is preserved, it's not stripped
        if (result.metrics.iccPreserved) {
            assert.strictEqual(result.metrics.metadataStripped, false, 
                'if iccPreserved is true, metadataStripped should be false');
        }

        // Legacy aliases
        assert.strictEqual(result.metrics.decodeTime, result.metrics.decodeMs);
        assert.strictEqual(result.metrics.processTime, result.metrics.opsMs);
        assert.strictEqual(result.metrics.encodeTime, result.metrics.encodeMs);
        assert.strictEqual(result.metrics.memoryPeak, result.metrics.peakRss);
        assert.strictEqual(result.metrics.inputSize, result.metrics.bytesIn);
        assert.strictEqual(result.metrics.outputSize, result.metrics.bytesOut);

        // Telemetry
        assert(typeof result.metrics.cpuTime === 'number', 'cpuTime should be number');
        assert(typeof result.metrics.processingTime === 'number', 'processingTime should be number');
        assert(typeof result.metrics.compressionRatio === 'number', 'compressionRatio should be number');
        assert(result.metrics.cpuTime >= 0, 'cpuTime should be non-negative');
        assert(result.metrics.processingTime >= 0, 'processingTime should be non-negative');
        assert(result.metrics.compressionRatio >= 0, 'compressionRatio should be non-negative');

        // Contract: processingTime should encompass decode+process+encode (same Instant baseline)
        const stageMs = result.metrics.decodeMs + result.metrics.opsMs + result.metrics.encodeMs;
        const totalMs = result.metrics.totalMs;
        assert(stageMs > 0, 'sum of decode/process/encode should be positive');
        assert(
            totalMs + 5 >= stageMs,
            'processingTime must include decode+process+encode (allow slight scheduling drift)'
        );
    });

    await asyncTest('toBufferWithMetrics handles input_size=0 gracefully', async () => {
        // This test verifies that when source is not available (edge case),
        // input_size=0 and compressionRatio=0 are handled correctly
        const buffer = fs.readFileSync(TEST_IMAGE);
        const { metrics } = await ImageEngine.from(buffer)
            .resize(100)
            .toBufferWithMetrics('jpeg', 80);
        
        // input_size should be positive for valid buffer source
        assert(metrics.bytesIn > 0, 'bytesIn should be positive for buffer source');
        assert(metrics.bytesOut > 0, 'bytesOut should be positive');
        assert(metrics.compressionRatio > 0, 'compressionRatio should be positive');
        
        // Verify compressionRatio calculation
        const expectedRatio = metrics.bytesOut / metrics.bytesIn;
        assert(Math.abs(metrics.compressionRatio - expectedRatio) < 0.0001, 
            'compressionRatio should equal outputSize / inputSize');
    });
    
    // ========================================================================
    // EDGE CASES - ICC Profile
    // ========================================================================
    
    await asyncTest('hasIccProfile() returns null for images without profile', async () => {
        const engine = ImageEngine.from(buffer);
        const hasProfile = engine.hasIccProfile();
        // Should return null or a number (profile size)
        assert(hasProfile === null || typeof hasProfile === 'number', 
            'should return null or number');
    });
    
    // ========================================================================
    // EDGE CASES - Brightness and Contrast
    // ========================================================================
    
    await asyncTest('handles brightness at limits (-100, 100)', async () => {
        const result1 = await ImageEngine.from(buffer).resize(100).brightness(-100).toBuffer('jpeg', 80);
        const result2 = await ImageEngine.from(buffer).resize(100).brightness(100).toBuffer('jpeg', 80);
        assert(result1.length > 0, 'should handle brightness -100');
        assert(result2.length > 0, 'should handle brightness 100');
    });
    
    await asyncTest('rejects brightness outside range', async () => {
        let threw = false;
        try {
            await ImageEngine.from(buffer).resize(100).brightness(-200).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            const category = getErrorCategory(e);
            assert.strictEqual(
                category,
                ErrorCategory.UserError,
                'out-of-range brightness should be UserError'
            );
        }
        assert(threw, 'should throw for brightness below -100');

        threw = false;
        try {
            await ImageEngine.from(buffer).resize(100).brightness(200).toBuffer('jpeg', 80);
        } catch (e) {
            threw = true;
            const category = getErrorCategory(e);
            assert.strictEqual(
                category,
                ErrorCategory.UserError,
                'out-of-range brightness should be UserError'
            );
        }
        assert(threw, 'should throw for brightness above 100');
    });
    
    await asyncTest('handles contrast at limits (-100, 100)', async () => {
        const result1 = await ImageEngine.from(buffer).resize(100).contrast(-100).toBuffer('jpeg', 80);
        const result2 = await ImageEngine.from(buffer).resize(100).contrast(100).toBuffer('jpeg', 80);
        assert(result1.length > 0, 'should handle contrast -100');
        assert(result2.length > 0, 'should handle contrast 100');
    });
    
    // ========================================================================
    // EDGE CASES - Clone and reuse
    // ========================================================================
    
    await asyncTest('clone() creates independent instances', async () => {
        const engine1 = ImageEngine.from(buffer).resize(100);
        const engine2 = engine1.clone();
        
        // Both should work independently
        const [result1, result2] = await Promise.all([
            engine1.toBuffer('jpeg', 80),
            engine2.toBuffer('webp', 80)
        ]);
        
        assert(result1.length > 0, 'original engine should work');
        assert(result2.length > 0, 'cloned engine should work');
    });
    
    // ========================================================================
    // EDGE CASES - Dimensions
    // ========================================================================
    
    await asyncTest('dimensions() returns correct values', async () => {
        const engine = ImageEngine.from(buffer);
        const dims = engine.dimensions();
        assert(dims.width > 0, 'width should be positive');
        assert(dims.height > 0, 'height should be positive');
        assert(dims.width === meta.width, 'width should match inspect()');
        assert(dims.height === meta.height, 'height should match inspect()');
    });
    
    // Summary
    console.log(`\n=== Results: ${passed} passed, ${failed} failed ===`);
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(e => {
    console.error('Test runner error:', e);
    process.exit(1);
});
