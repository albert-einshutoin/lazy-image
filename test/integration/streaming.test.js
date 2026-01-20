/**
 * Streaming transform smoke test.
 * Streams a fixture JPEG through createImageTransform -> expects resized JPEG output.
 */

const fs = require('fs');
const assert = require('assert');
const { pipeline } = require('stream/promises');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { createStreamingPipeline } = require('../../streaming/pipeline');
const { inspect } = require(resolveRoot('index'));

const INPUT = resolveFixture('test_input.jpg');

async function run() {
    console.log('=== Streaming Transform Tests ===');

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

run().catch((err) => {
    console.error(err);
    process.exit(1);
});
