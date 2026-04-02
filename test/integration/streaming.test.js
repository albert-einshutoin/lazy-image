/**
 * Streaming transform smoke test.
 * Streams a fixture JPEG through createImageTransform -> expects resized JPEG output.
 */

const fs = require('fs');
const os = require('os');
const path = require('path');
const assert = require('assert');
const { pipeline } = require('stream/promises');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine, createStreamingPipeline, inspect } = require(resolveRoot('index'));

const INPUT = resolveFixture('test_input.jpg');
const LARGE_INPUT = resolveFixture('test_4.5MB_5000x5000.png');

// Create a non-trivial fixture (200x100) for tests that need meaningful
// dimension assertions.  The default test_input.jpg is 1x1.
let RECT_INPUT;
async function ensureRectFixture() {
    if (RECT_INPUT) return RECT_INPUT;
    const squareBuf = await ImageEngine.from(fs.readFileSync(INPUT))
        .resize(200)
        .toBuffer('jpeg', 90);
    const rectBuf = await ImageEngine.from(squareBuf)
        .crop(0, 0, 200, 100)
        .toBuffer('jpeg', 90);
    RECT_INPUT = path.join(os.tmpdir(), 'streaming-test-rect.jpg');
    fs.writeFileSync(RECT_INPUT, rectBuf);
    return RECT_INPUT;
}

async function run() {
    console.log('=== Streaming Transform Tests ===');

    await test_basic_resize();
    await test_large_file_resize();
    await test_metrics_callback();
    await test_error_propagation();
    await test_multiple_ops();
    await test_png_output();
    await test_avif_output();
    await test_destroy_mid_stream();
    await test_empty_input();
}

async function test_basic_resize() {
    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 82,
        ops: [{ op: 'resize', width: 400, height: null, fit: 'inside' }],
    });

    const source = fs.createReadStream(INPUT);
    await pipeline(source, writable);

    const chunks = [];
    for await (const chunk of readable) chunks.push(chunk);
    const output = Buffer.concat(chunks);
    const meta = inspect(output);

    assert(meta.width <= 400, 'width should be <= 400');
    assert(meta.height > 0, 'height should be > 0');
    assert(meta.format === 'jpeg', 'format should be jpeg');

    console.log('✅ streaming resize -> jpeg passed');
}

async function test_large_file_resize() {
    const { writable, readable } = createStreamingPipeline({
        format: 'webp',
        quality: 80,
        ops: [{ op: 'resize', width: 1600, height: null, fit: 'inside' }],
    });

    const source = fs.createReadStream(LARGE_INPUT);
    await pipeline(source, writable);

    const outChunks = [];
    for await (const chunk of readable) outChunks.push(chunk);
    const output = Buffer.concat(outChunks);
    const meta = inspect(output);

    assert(meta.width <= 1600, 'large resize width should be capped');
    assert(meta.height > 0, 'large resize height should be > 0');
    assert(meta.format === 'webp', 'format should be webp');

    console.log('✅ streaming large resize -> webp passed');
}

async function test_metrics_callback() {
    let seenMetrics = null;
    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 80,
        ops: [{ op: 'resize', width: 320, height: null, fit: 'inside' }],
        onMetrics(metrics) {
            seenMetrics = metrics;
        },
    });

    const source = fs.createReadStream(INPUT);
    await pipeline(source, writable);

    const chunks = [];
    for await (const chunk of readable) chunks.push(chunk);
    const output = Buffer.concat(chunks);
    const meta = inspect(output);

    assert(meta.width <= 320, 'metrics callback output width should be <= 320');
    assert(seenMetrics, 'onMetrics should be called');
    assert(seenMetrics.totalMs >= 0, 'onMetrics should receive totalMs');
    assert(seenMetrics.formatOut === 'jpeg', 'onMetrics should report requested format');

    console.log('✅ streaming metrics callback passed');
}

async function test_error_propagation() {
    // feed invalid bytes to ensure error surfaces on readable
    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 80,
    });

    // close writable with garbage
    writable.write(Buffer.from([0, 1, 2, 3]));
    writable.end();

    let failed = false;
    try {
        for await (const _ of readable) {
            // should not reach
        }
    } catch (err) {
        failed = true;
        assert(err, 'error should be thrown for invalid data');
    }
    assert(failed, 'error path should be exercised');
    console.log('✅ streaming error propagation passed');
}

async function test_multiple_ops() {
    // Use a 200x100 fixture so resize and rotate produce verifiable results.
    const rectPath = await ensureRectFixture();

    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 80,
        ops: [
            { op: 'resize', width: 150, height: null, fit: 'inside' },
            { op: 'rotate', degrees: 90 },
            // grayscale is included to exercise the code path; the JS inspect()
            // API does not expose channel count, so it is not directly verifiable.
            { op: 'grayscale' },
        ],
    });

    const source = fs.createReadStream(rectPath);
    await pipeline(source, writable);

    const chunks = [];
    for await (const chunk of readable) chunks.push(chunk);
    const output = Buffer.concat(chunks);
    const meta = inspect(output);

    // After resize(150, inside) on 200x100: -> 150x75
    // After rotate(90): width and height swap -> 75x150
    assert(meta.width <= 150, `width ${meta.width} should be <= 150 after resize`);
    assert(meta.height <= 150, `height ${meta.height} should be <= 150 after resize`);
    assert(meta.width !== meta.height, `dimensions should differ after rotate on non-square input, got ${meta.width}x${meta.height}`);
    assert(meta.format === 'jpeg', 'format should be jpeg');

    console.log('✅ streaming multiple ops passed');
}

async function test_png_output() {
    const { writable, readable } = createStreamingPipeline({
        format: 'png',
        ops: [{ op: 'resize', width: 200, height: null, fit: 'inside' }],
    });

    const source = fs.createReadStream(INPUT);
    await pipeline(source, writable);

    const chunks = [];
    for await (const chunk of readable) chunks.push(chunk);
    const output = Buffer.concat(chunks);
    const meta = inspect(output);

    assert(meta.width <= 200, 'png output width should be <= 200');
    assert(meta.height > 0, 'png output height should be > 0');
    assert(meta.format === 'png', 'format should be png');

    console.log('✅ streaming resize -> png passed');
}

async function test_avif_output() {
    const { writable, readable } = createStreamingPipeline({
        format: 'avif',
        quality: 30,
        ops: [{ op: 'resize', width: 200, height: null, fit: 'inside' }],
    });

    const source = fs.createReadStream(INPUT);
    await pipeline(source, writable);

    const chunks = [];
    for await (const chunk of readable) chunks.push(chunk);
    const output = Buffer.concat(chunks);

    // AVIF validation via ISOBMFF ftyp box (accept any AVIF-compatible brand)
    assert(output.length > 12, 'avif output should be non-empty');
    const ftyp = output.toString('ascii', 4, 8);
    assert.strictEqual(ftyp, 'ftyp', 'avif output should start with ftyp box');
    const brand = output.toString('ascii', 8, 12);
    assert(
        ['avif', 'avis', 'mif1'].includes(brand),
        `avif output should have AVIF-compatible brand, got: ${brand}`,
    );

    console.log('✅ streaming resize -> avif passed');
}

async function test_destroy_mid_stream() {
    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 80,
        ops: [{ op: 'resize', width: 400, height: null, fit: 'inside' }],
    });

    // Destroy the writable with an error to simulate client abort.
    // No prior write() — this isolates the abort path from decode errors.
    writable.destroy(new Error('client aborted'));

    // The readable should receive the error
    const result = await new Promise((resolve) => {
        const timer = setTimeout(() => resolve('timeout'), 3000);
        readable.on('error', () => {
            clearTimeout(timer);
            resolve('error');
        });
        readable.on('end', () => {
            clearTimeout(timer);
            resolve('end');
        });
        readable.on('close', () => {
            clearTimeout(timer);
            resolve('close');
        });
        readable.resume(); // drain to trigger events
    });

    // The readable should propagate the error from the destroyed writable
    assert(
        result === 'error',
        `destroy mid-stream should propagate error to readable, got: ${result}`,
    );

    console.log('✅ streaming destroy mid-stream passed');
}

async function test_empty_input() {
    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        quality: 80,
    });

    // Send zero bytes
    writable.end(Buffer.alloc(0));

    const result = await new Promise((resolve) => {
        const timer = setTimeout(() => resolve('timeout'), 5000);
        let errorSeen = false;
        readable.on('error', () => {
            clearTimeout(timer);
            errorSeen = true;
            resolve('error');
        });
        readable.on('end', () => {
            clearTimeout(timer);
            resolve(errorSeen ? 'error' : 'end');
        });
        readable.on('close', () => {
            clearTimeout(timer);
            if (!errorSeen) resolve('close');
        });
        readable.resume(); // drain to trigger events
    });

    assert(
        result === 'error',
        `empty input should result in an error, got: ${result}`,
    );

    console.log('✅ streaming empty input error passed');
}

run().catch((err) => {
    console.error(err);
    process.exit(1);
});
