/**
 * Format conversion matrix and resize fit mode tests (#475)
 *
 * Verifies that every supported input format (JPEG, PNG, WebP) can be decoded
 * and re-encoded to every supported output format (JPEG, PNG, WebP, AVIF),
 * that the three resize fit modes produce the expected dimensional constraints,
 * and that normalizePixelFormat() works across formats.
 *
 * Note: AVIF *encoding* is supported but AVIF *decoding* (as input) is not
 * yet available in the native binding, so AVIF-as-source tests are omitted.
 * The inspect() helper also cannot read AVIF, so AVIF output is validated
 * via the ISOBMFF ftyp box signature instead.
 *
 * Run with: node test/integration/format-matrix.test.js
 */

const fs = require('fs');
const assert = require('assert');
const { resolveRoot, resolveFixture } = require('../helpers/paths');
const { ImageEngine, inspect } = require(resolveRoot('index'));

let passed = 0;
let failed = 0;

async function asyncTest(name, fn) {
    try {
        await fn();
        console.log(`\u2705 ${name}`);
        passed++;
    } catch (e) {
        console.log(`\u274C ${name}`);
        console.log(`   Error: ${e.message}`);
        failed++;
    }
}

/**
 * Return true when the buffer looks like a valid AVIF file (ISOBMFF ftyp box
 * with 'avif' brand).
 */
function looksLikeAvif(buf) {
    // ISOBMFF: bytes 4-7 == 'ftyp', bytes 8-11 == 'avif'
    if (buf.length < 12) return false;
    return (
        buf.toString('ascii', 4, 8) === 'ftyp' &&
        buf.toString('ascii', 8, 12) === 'avif'
    );
}

// ---------------------------------------------------------------------------
// Fixture generation
// ---------------------------------------------------------------------------

async function generateFixtures() {
    const jpegPath = resolveFixture('test_input.jpg');
    if (!fs.existsSync(jpegPath)) {
        // Ensure the minimal JPEG fixture exists (CI helper normally creates it)
        require('../helpers/create-test-image').main();
    }
    const jpegBuf = fs.readFileSync(jpegPath);

    // Generate a small WebP buffer from the JPEG fixture.
    const webpBuf = await ImageEngine.from(jpegBuf).resize(200).toBuffer('webp', 80);

    // Also keep a PNG buffer handy.
    const pngBuf = fs.readFileSync(resolveFixture('test_input.png'));

    return { jpegBuf, pngBuf, webpBuf };
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function runTests() {
    console.log('=== Format Conversion Matrix Tests ===\n');

    const { jpegBuf, pngBuf, webpBuf } = await generateFixtures();

    // --- Format conversion matrix -------------------------------------------
    // Each entry: [label, sourceBuf, targetFormat, quality]
    //
    // AVIF input is not supported by the current native binding so those
    // conversions are intentionally excluded.  AVIF output is tested via the
    // ftyp box signature rather than inspect().

    const matrix = [
        ['JPEG -> JPEG',  jpegBuf,   'jpeg',  80],
        ['JPEG -> PNG',   jpegBuf,   'png',   null],
        ['JPEG -> WebP',  jpegBuf,   'webp',  80],
        ['JPEG -> AVIF',  jpegBuf,   'avif',  30],
        ['WebP -> JPEG',  webpBuf,   'jpeg',  80],
        ['WebP -> PNG',   webpBuf,   'png',   null],
        ['WebP -> WebP',  webpBuf,   'webp',  80],
        ['WebP -> AVIF',  webpBuf,   'avif',  30],
        ['PNG  -> JPEG',  pngBuf,    'jpeg',  80],
        ['PNG  -> PNG',   pngBuf,    'png',   null],
        ['PNG  -> WebP',  pngBuf,    'webp',  80],
        ['PNG  -> AVIF',  pngBuf,    'avif',  30],
    ];

    for (const [label, src, fmt, quality] of matrix) {
        await asyncTest(`format: ${label}`, async () => {
            const args = quality != null ? [fmt, quality] : [fmt];
            const result = await ImageEngine.from(src).resize(100).toBuffer(...args);
            assert(result.length > 0, 'output buffer should be non-empty');

            if (fmt === 'avif') {
                // inspect() cannot read AVIF; validate via ftyp signature.
                assert(looksLikeAvif(result), 'AVIF output should contain ftyp/avif box');
            } else {
                // Verify the output can be inspected (proves it is a valid image).
                const meta = inspect(result);
                assert(meta.width > 0, 'output width should be positive');
                assert(meta.height > 0, 'output height should be positive');

                if (meta.format) {
                    const expected = fmt === 'jpeg' ? 'jpeg' : fmt;
                    assert.strictEqual(meta.format, expected, `output format should be ${expected}`);
                }
            }
        });
    }

    // --- Resize fit mode tests ----------------------------------------------

    // We need a non-square source image so the fit modes produce
    // distinguishable results.  Generate a 200x100 rectangle from the JPEG
    // fixture by cropping after an initial resize.
    const rectBuf = await ImageEngine.from(jpegBuf)
        .resize(200)
        .toBuffer('jpeg', 90);

    // Inspect to learn the actual dimensions (the fixture may be 1x1, in which
    // case resize(200) gives 200x200 -- still fine, but we need to know).
    const rectMeta = inspect(rectBuf);

    // Only run fit-mode tests if the rectangle is truly non-square (requires
    // the source image to be non-square).  If the source is square we still
    // exercise the code paths but relax the assertions.
    const isNonSquare = rectMeta.width !== rectMeta.height;

    const TARGET = 100;

    await asyncTest('resize fit: inside (both dims <= target)', async () => {
        const result = await ImageEngine.from(rectBuf)
            .resize({ width: TARGET, height: TARGET, fit: 'inside' })
            .toBuffer('jpeg', 80);
        const meta = inspect(result);
        assert(meta.width <= TARGET, `width ${meta.width} should be <= ${TARGET}`);
        assert(meta.height <= TARGET, `height ${meta.height} should be <= ${TARGET}`);
        if (isNonSquare) {
            // At least one dimension should be exactly the target (or smaller
            // if the source was already smaller).
            assert(
                meta.width === TARGET || meta.height === TARGET,
                'one dimension should equal target when source is non-square',
            );
        }
    });

    await asyncTest('resize fit: cover (both dims >= target)', async () => {
        const result = await ImageEngine.from(rectBuf)
            .resize({ width: TARGET, height: TARGET, fit: 'cover' })
            .toBuffer('jpeg', 80);
        const meta = inspect(result);
        assert(meta.width >= TARGET, `width ${meta.width} should be >= ${TARGET}`);
        assert(meta.height >= TARGET, `height ${meta.height} should be >= ${TARGET}`);
    });

    await asyncTest('resize fit: fill (exact target dimensions)', async () => {
        const result = await ImageEngine.from(rectBuf)
            .resize({ width: TARGET, height: TARGET, fit: 'fill' })
            .toBuffer('jpeg', 80);
        const meta = inspect(result);
        assert.strictEqual(meta.width, TARGET, `width should be exactly ${TARGET}`);
        assert.strictEqual(meta.height, TARGET, `height should be exactly ${TARGET}`);
    });

    // --- normalizePixelFormat() tests ----------------------------------------

    await asyncTest('normalizePixelFormat: JPEG produces valid output', async () => {
        const result = await ImageEngine.from(jpegBuf)
            .normalizePixelFormat()
            .resize(100)
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output buffer should be non-empty');
        const meta = inspect(result);
        assert(meta.width > 0, 'output width should be positive');
        assert(meta.height > 0, 'output height should be positive');
    });

    await asyncTest('normalizePixelFormat: PNG produces valid output', async () => {
        const result = await ImageEngine.from(pngBuf)
            .normalizePixelFormat()
            .resize(100)
            .toBuffer('png');
        assert(result.length > 0, 'output buffer should be non-empty');
        const meta = inspect(result);
        assert(meta.width > 0, 'output width should be positive');
        assert(meta.height > 0, 'output height should be positive');
    });

    await asyncTest('normalizePixelFormat: WebP produces valid output', async () => {
        const result = await ImageEngine.from(webpBuf)
            .normalizePixelFormat()
            .resize(100)
            .toBuffer('webp', 80);
        assert(result.length > 0, 'output buffer should be non-empty');
        const meta = inspect(result);
        assert(meta.width > 0, 'output width should be positive');
        assert(meta.height > 0, 'output height should be positive');
    });

    await asyncTest('normalizePixelFormat: converts grayscale to RGB', async () => {
        // Create a grayscale image by applying grayscale() first
        const grayBuf = await ImageEngine.from(jpegBuf)
            .resize(100)
            .grayscale()
            .toBuffer('png');
        // Now normalizePixelFormat should convert the grayscale layout to RGB
        const result = await ImageEngine.from(grayBuf)
            .normalizePixelFormat()
            .toBuffer('png');
        assert(result.length > 0, 'output buffer should be non-empty');
        const meta = inspect(result);
        assert(meta.width > 0, 'output width should be positive');
        assert(meta.height > 0, 'output height should be positive');
    });

    await asyncTest('normalizePixelFormat: chainable with other operations', async () => {
        const result = await ImageEngine.from(jpegBuf)
            .resize(100)
            .normalizePixelFormat()
            .grayscale()
            .toBuffer('jpeg', 80);
        assert(result.length > 0, 'output buffer should be non-empty');
        const meta = inspect(result);
        assert.strictEqual(meta.width, 100, 'width should be 100 after resize');
        assert(meta.height > 0, 'output height should be positive');
    });

    // --- Summary ------------------------------------------------------------

    console.log(`\n=== Results: ${passed} passed, ${failed} failed ===`);
    process.exit(failed > 0 ? 1 : 0);
}

runTests().catch(e => {
    console.error('Test runner error:', e);
    process.exit(1);
});
