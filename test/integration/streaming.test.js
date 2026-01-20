/**
 * Streaming transform smoke test.
 * Streams a fixture JPEG through createImageTransform -> expects resized JPEG output.
 */

const fs = require('fs');
const assert = require('assert');
const { pipeline } = require('stream/promises');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { createStreamingPipeline, inspect } = require(resolveRoot('index'));

const INPUT = resolveFixture('test_input.jpg');
const LARGE_INPUT = resolveFixture('test_4.5MB_5000x5000.png');

async function run() {
    console.log('=== Streaming Transform Tests ===');

    await test_basic_resize();
    await test_large_file_resize();
    await test_error_propagation();
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

run().catch((err) => {
    console.error(err);
    process.exit(1);
});
