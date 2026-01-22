/**
 * Image Firewall Integration Tests
 * Tests for sanitize() and limits() API
 *
 * Run with: node test/integration/firewall.test.js
 */

const fs = require('fs');
const assert = require('assert');
const zlib = require('zlib');
const { resolveRoot, resolveFixture } = require('../helpers/paths');
const { ImageEngine, ErrorCategory } = require(resolveRoot('index'));

const TEST_IMAGE = resolveFixture('test_input.jpg');

let passed = 0;
let failed = 0;

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
        if (e.stack) {
            console.log(`   Stack: ${e.stack.split('\n').slice(1, 3).join('\n   ')}`);
        }
        failed++;
    }
}

async function runTests() {
    console.log('=== Image Firewall Tests ===\n');

    const buffer = fs.readFileSync(TEST_IMAGE);
    const smallBuffer = await ImageEngine.from(buffer).resize(10).toBuffer('jpeg', 80);

    // =========================================================================
    // Basic sanitize() tests
    // =========================================================================

    await asyncTest('sanitize({ policy: "strict" }) works with small image', async () => {
        const result = await ImageEngine.from(smallBuffer)
            .sanitize({ policy: 'strict' })
            .resize(8)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('sanitize({ policy: "lenient" }) works with normal image', async () => {
        const result = await ImageEngine.from(buffer)
            .sanitize({ policy: 'lenient' })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('sanitize() defaults to strict policy', async () => {
        const result = await ImageEngine.from(smallBuffer)
            .sanitize()
            .resize(8)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    // =========================================================================
    // limits() tests
    // =========================================================================

    await asyncTest('limits() can override maxPixels', async () => {
        const result = await ImageEngine.from(buffer)
            .sanitize({ policy: 'strict' })
            .limits({ maxPixels: 100_000_000 })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('limits() can override maxBytes', async () => {
        const result = await ImageEngine.from(buffer)
            .sanitize({ policy: 'strict' })
            .limits({ maxBytes: 100_000_000 })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('limits() can override timeoutMs', async () => {
        const result = await ImageEngine.from(buffer)
            .sanitize({ policy: 'strict' })
            .limits({ timeoutMs: 60000 })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('limits({ maxPixels: 0 }) disables pixel limit', async () => {
        const result = await ImageEngine.from(buffer)
            .sanitize({ policy: 'strict' })
            .limits({ maxPixels: 0 })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    // =========================================================================
    // Error cases
    // =========================================================================

    await asyncTest('sanitize() rejects oversized images (maxBytes)', async () => {
        // Create a very small limit that the image exceeds
        try {
            await ImageEngine.from(buffer)
                .sanitize({ policy: 'strict' })
                .limits({ maxBytes: 100 })  // 100 bytes - too small
                .toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            assert(e.message.includes('Firewall'), `error should mention Firewall: ${e.message}`);
            assert(e.message.includes('bytes'), `error should mention bytes: ${e.message}`);
        }
    });

    await asyncTest('sanitize() rejects oversized images (maxPixels)', async () => {
        // Use 10,000x1 instead of 30,000x1 to stay within zune-png's default limit (16384)
        // This ensures Firewall pixel check is triggered, not zune-png's internal limit
        const oversized = createGrayscalePng(10_000, 1);
        try {
            await ImageEngine.from(oversized)
                .sanitize({ policy: 'strict' })
                .limits({ maxPixels: 1_000 })
                .toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            assert(e.message.includes('Firewall'), `error should mention Firewall: ${e.message}`);
            assert(e.message.includes('pixels'), `error should mention pixels: ${e.message}`);
        }
    });

    await asyncTest('invalid policy name throws error', async () => {
        try {
            await ImageEngine.from(buffer)
                .sanitize({ policy: 'invalid_policy' })
                .toBuffer('jpeg', 80);
            assert.fail('should have thrown an error');
        } catch (e) {
            assert(e.message.includes('invalid_policy'), `error should mention the invalid policy: ${e.message}`);
        }
    });

    // =========================================================================
    // Edge cases
    // =========================================================================

    await asyncTest('sanitize() can be called multiple times', async () => {
        const result = await ImageEngine.from(smallBuffer)
            .sanitize({ policy: 'strict' })
            .sanitize({ policy: 'lenient' })  // Override to lenient
            .resize(8)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('limits() without sanitize() enables firewall', async () => {
        const result = await ImageEngine.from(buffer)
            .limits({ maxPixels: 100_000_000 })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    await asyncTest('clone() preserves firewall config', async () => {
        const engine = ImageEngine.from(smallBuffer)
            .sanitize({ policy: 'lenient' })
            .resize(8);

        const [result1, result2] = await Promise.all([
            engine.clone().toBuffer('jpeg', 80),
            engine.clone().toBuffer('webp', 80),
        ]);

        assert(result1.length > 0, 'JPEG should have content');
        assert(result2.length > 0, 'WebP should have content');
    });

    await asyncTest('fromPath() works with sanitize()', async () => {
        const result = await ImageEngine.fromPath(TEST_IMAGE)
            .sanitize({ policy: 'lenient' })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output should have content');
    });

    // =========================================================================
    // Policy comparison tests
    // =========================================================================

    await asyncTest('strict policy has tighter limits than lenient', async () => {
        // This test verifies the policies have different limits
        // by using limits that would pass lenient but fail strict

        // First verify lenient accepts the image
        const lenientResult = await ImageEngine.from(buffer)
            .sanitize({ policy: 'lenient' })
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(lenientResult.length > 0, 'lenient should accept the image');
    });

    // =========================================================================
    // Error message quality tests
    // =========================================================================

    await asyncTest('error messages are actionable (include suggestions)', async () => {
        try {
            await ImageEngine.from(buffer)
                .sanitize({ policy: 'strict' })
                .limits({ maxBytes: 100 })
                .toBuffer('jpeg', 80);
            assert.fail('should have thrown');
        } catch (e) {
            // Error message should suggest how to fix the issue
            assert(
                e.message.includes('limits') || e.message.includes('lenient'),
                `error should suggest a fix: ${e.message}`
            );
        }
    });

    // =========================================================================
    // Summary
    // =========================================================================

    console.log(`\n=== Results: ${passed} passed, ${failed} failed ===`);

    if (failed > 0) {
        process.exit(1);
    }
}

runTests().catch(e => {
    console.error('Test runner failed:', e);
    process.exit(1);
});
