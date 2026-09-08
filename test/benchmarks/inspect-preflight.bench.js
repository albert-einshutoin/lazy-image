'use strict';

const { performance } = require('node:perf_hooks');
const { inspectFile } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

const cases = [
    ['jpeg-3.2mb', resolveFixture('test_3.2MB_5000x5000.jpg')],
    ['png-4.5mb', resolveFixture('test_4.5MB_5000x5000.png')],
    ['webp-1.4mb', resolveFixture('test_1.4MB_5000x5000.webp')],
];

const iterations = Number(process.env.INSPECT_BENCH_ITERATIONS || 50);
if (!Number.isInteger(iterations) || iterations < 1) {
    throw new RangeError('INSPECT_BENCH_ITERATIONS must be a positive integer');
}

const results = [];

for (const [name, path] of cases) {
    for (let warmup = 0; warmup < 5; warmup++) inspectFile(path);
    const started = performance.now();
    let metadata;
    for (let iteration = 0; iteration < iterations; iteration++) {
        metadata = inspectFile(path);
    }
    const elapsedMs = performance.now() - started;
    results.push({
        name,
        iterations,
        meanMs: Number((elapsedMs / iterations).toFixed(3)),
        width: metadata.width,
        height: metadata.height,
        hasAlpha: metadata.hasAlpha,
        isAnimated: metadata.isAnimated,
    });
}

console.log(JSON.stringify({ schemaVersion: 1, results }, null, 2));
