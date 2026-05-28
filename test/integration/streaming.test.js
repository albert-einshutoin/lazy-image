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
    await test_async_process_error_propagation();
    await test_multiple_ops();
    await test_png_output();
    await test_avif_output();
    await test_destroy_mid_stream();
    await test_empty_input();
    await test_cleanup_error_reporting();
    await test_memory_pressure_concurrent_streams();
    await test_memory_pressure_rss_bound();
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

async function test_async_process_error_propagation() {
    class FailingEngine {
        static fromPath() {
            return new FailingEngine();
        }

        async toFile() {
            throw new Error('encode exploded after finish');
        }
    }

    const { writable, readable } = createStreamingPipeline({
        format: 'jpeg',
        ImageEngine: FailingEngine,
    });

    writable.end(Buffer.from('not-an-image-but-engine-is-mocked'));

    let capturedError = null;
    try {
        for await (const _chunk of readable) {
            // should not reach
        }
    } catch (error) {
        capturedError = error;
    }

    assert(capturedError, 'async process failure should reach readable');
    assert.match(capturedError.message, /encode exploded after finish/);

    console.log('✅ streaming async process error propagation passed');
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

async function test_cleanup_error_reporting() {
    const originalRm = fs.rm;
    const cleanupErrors = [];
    fs.rm = function patchedRm(target, options, callback) {
        const cb = typeof options === 'function' ? options : callback;
        process.nextTick(() => cb(new Error(`forced cleanup failure: ${target}`)));
    };

    try {
        const { writable, readable } = createStreamingPipeline({
            format: 'jpeg',
            quality: 80,
            onCleanupError(err) {
                cleanupErrors.push(err);
            },
        });

        writable.end(Buffer.from([0, 1, 2, 3]));

        await new Promise((resolve) => {
            readable.on('error', resolve);
            readable.resume();
        });
    } finally {
        fs.rm = originalRm;
    }

    assert(
        cleanupErrors.length > 0,
        'cleanup failures should be reported instead of silently swallowed',
    );

    console.log('✅ streaming cleanup error reporting passed');
}

/**
 * Memory pressure under concurrency.
 *
 * Run several streaming pipelines in parallel against the same large fixture
 * (5000×5000 PNG → ~96 MB decoded RGBA). The internal weighted memory semaphore
 * is expected to serialize/throttle the heavy decode+encode work so all
 * pipelines finish successfully without crashes, panics, or OOM.
 *
 * This is a regression guard for #480: the streaming layer must compose with
 * the engine's memory-bounded concurrency, not bypass it.
 */
async function test_memory_pressure_concurrent_streams() {
    const N = 4;

    const tasks = Array.from({ length: N }, async (_unused, i) => {
        const { writable, readable } = createStreamingPipeline({
            format: 'webp',
            quality: 80,
            ops: [{ op: 'resize', width: 800, height: null, fit: 'inside' }],
        });

        const source = fs.createReadStream(LARGE_INPUT);
        await pipeline(source, writable);

        const chunks = [];
        for await (const chunk of readable) chunks.push(chunk);
        const output = Buffer.concat(chunks);
        const meta = inspect(output);

        return { i, bytes: output.length, width: meta.width, format: meta.format };
    });

    const results = await Promise.all(tasks);

    for (const r of results) {
        assert(r.bytes > 0, `concurrent pipeline ${r.i} produced empty output`);
        assert(
            r.width > 0 && r.width <= 800,
            `concurrent pipeline ${r.i} did not resize correctly (width=${r.width})`,
        );
        assert(
            r.format === 'webp',
            `concurrent pipeline ${r.i} format mismatch (got ${r.format})`,
        );
    }

    // Sanity: all N outputs should be roughly the same size (deterministic input + ops).
    // Allow ±10% to absorb encoder timing variance.
    const sizes = results.map((r) => r.bytes);
    const minSize = Math.min(...sizes);
    const maxSize = Math.max(...sizes);
    assert(
        maxSize - minSize <= maxSize * 0.1,
        `concurrent outputs diverged in size (min=${minSize}, max=${maxSize}) — possible cross-pipeline contamination`,
    );

    console.log(`✅ streaming memory pressure (${N} concurrent) passed`);
}

/**
 * RSS bound under a heavy single-pipeline workload.
 *
 * The streaming pipeline's promise is "memory stays ~O(1) while disk usage
 * mirrors input/output sizes". This test runs one large pipeline and checks
 * that the process RSS growth stays well below the decoded-RGBA size of the
 * input (5000×5000×4 ≈ 96 MB).
 *
 * Threshold rationale: the engine's decode + encode working set plus Node's
 * baseline ~30 MB and rayon thread allocations should keep RSS growth under
 * ~250 MB even with a 96 MB decoded image. A higher number means the
 * streaming layer is double-buffering input bytes in V8 — a regression.
 *
 * Sampling: a periodic interval samples RSS *across the entire pipeline
 * lifecycle* — input staging, decode+encode (which finishes inside
 * processImage() before the first chunk arrives on the readable), and
 * readback. Sampling only during chunk consumption would miss the heavy
 * transformation peak entirely.
 */
async function test_memory_pressure_rss_bound() {
    if (global.gc) global.gc();
    const rssStart = process.memoryUsage().rss;
    let peakRss = rssStart;

    // Sample at 5 ms — small enough to catch the short decode+encode burst
    // (typically tens of ms on a 5000×5000 PNG → JPEG resize), large enough
    // to keep sampling overhead negligible relative to the work it observes.
    const SAMPLE_INTERVAL_MS = 5;
    const sampler = setInterval(() => {
        const current = process.memoryUsage().rss;
        if (current > peakRss) peakRss = current;
    }, SAMPLE_INTERVAL_MS);
    // Don't keep the event loop alive solely for the sampler.
    if (typeof sampler.unref === 'function') sampler.unref();

    try {
        const { writable, readable } = createStreamingPipeline({
            format: 'jpeg',
            quality: 80,
            ops: [{ op: 'resize', width: 1200, height: null, fit: 'inside' }],
        });

        const source = fs.createReadStream(LARGE_INPUT);
        await pipeline(source, writable);

        const chunks = [];
        for await (const chunk of readable) {
            chunks.push(chunk);
            // Also sample inline — chunk-driven sampling is cheap and
            // gives us extra resolution during readback.
            const current = process.memoryUsage().rss;
            if (current > peakRss) peakRss = current;
        }
        const output = Buffer.concat(chunks);
        assert(output.length > 0, 'rss-bound pipeline produced empty output');
    } finally {
        clearInterval(sampler);
    }

    // Final snapshot in case the last sample landed just before the peak.
    const finalRss = process.memoryUsage().rss;
    if (finalRss > peakRss) peakRss = finalRss;

    const rssDeltaMb = (peakRss - rssStart) / (1024 * 1024);
    // Generous ceiling — we mainly want to catch a regression where the entire
    // input file gets staged in the V8 heap or duplicated in memory.
    const LIMIT_MB = 250;
    assert(
        rssDeltaMb < LIMIT_MB,
        `streaming RSS growth exceeded budget: delta=${rssDeltaMb.toFixed(1)} MB, limit=${LIMIT_MB} MB`,
    );

    console.log(
        `✅ streaming memory pressure RSS bound passed (delta=${rssDeltaMb.toFixed(1)} MB < ${LIMIT_MB} MB)`,
    );
}

run().catch((err) => {
    console.error(err);
    process.exit(1);
});
