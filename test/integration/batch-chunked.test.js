const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { ImageEngine, processBatchChunked } = require('../..');
const { resolveFixture, resolveTemp } = require('../helpers/paths');

const SOURCE = resolveFixture('test_100KB_1057x1057.jpg');
const ROOT = resolveTemp('batch-chunked');

function resetDir(dir) {
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function makeInputs(name, count) {
  const dir = resetDir(path.join(ROOT, name, 'inputs'));
  return Array.from({ length: count }, (_, index) => {
    const target = path.join(dir, `input-${index}.jpg`);
    fs.copyFileSync(SOURCE, target);
    return target;
  });
}

function resultShape(results) {
  return results.map((result) => ({
    source: result.source,
    success: result.success,
    outputFile: result.outputPath ? path.basename(result.outputPath) : undefined,
    errorCode: result.errorCode,
  }));
}

async function assertAbortError(promise) {
  await assert.rejects(promise, (error) => {
    assert.equal(error.name, 'AbortError');
    return true;
  });
}

async function test(name, fn) {
  try {
    await fn();
    console.log(`✅ ${name}`);
  } catch (error) {
    console.log(`❌ ${name}`);
    throw error;
  }
}

async function main() {
  assert.equal(typeof processBatchChunked, 'function');

  await test('chunks inputs, preserves order, and reports progress', async () => {
    const inputs = makeInputs('progress', 5);
    const outputDir = resetDir(path.join(ROOT, 'progress', 'out'));
    const progress = [];

    const results = await processBatchChunked(inputs, outputDir, {
      format: 'webp',
      quality: 80,
      chunkSize: 2,
      onProgress: (event) => progress.push(event),
    });

    assert.equal(results.length, 5);
    assert.deepEqual(results.map((result) => result.source), inputs);
    assert.deepEqual(progress.map((event) => event.completed), [2, 4, 5]);
    assert.deepEqual(progress.map((event) => event.total), [5, 5, 5]);
    assert.deepEqual(progress.map((event) => event.failed), [0, 0, 0]);
    assert.deepEqual(progress.map((event) => event.lastChunk.length), [2, 2, 1]);
    assert(results.every((result) => result.success));
    for (const result of results) {
      assert(result.outputPath, 'successful result should expose outputPath');
      assert(fs.existsSync(result.outputPath), `output should exist: ${result.outputPath}`);
    }
  });

  await test('matches processBatch result order and output names', async () => {
    const inputs = makeInputs('match-process-batch', 4);
    const directOut = resetDir(path.join(ROOT, 'match-process-batch', 'direct'));
    const chunkedOut = resetDir(path.join(ROOT, 'match-process-batch', 'chunked'));
    const options = { format: 'jpeg', quality: 82, concurrency: 1 };

    const direct = await ImageEngine.fromPath(inputs[0]).processBatch(inputs, directOut, options);
    const chunked = await processBatchChunked(inputs, chunkedOut, { ...options, chunkSize: 2 });

    assert.deepEqual(resultShape(chunked), resultShape(direct));
  });

  await test('delegates single default-size chunk without progress callback overhead', async () => {
    const inputs = makeInputs('delegated', 3);
    const outputDir = resetDir(path.join(ROOT, 'delegated', 'out'));
    let progressCalls = 0;

    const results = await processBatchChunked(inputs, outputDir, {
      format: 'webp',
      quality: 80,
      onProgress: () => {
        progressCalls += 1;
      },
    });

    assert.equal(results.length, 3);
    assert.equal(progressCalls, 0);
    assert(results.every((result) => result.success));
  });

  await test('aborts at chunk boundaries and keeps completed chunk outputs', async () => {
    const inputs = makeInputs('abort-after-first-chunk', 4);
    const outputDir = resetDir(path.join(ROOT, 'abort-after-first-chunk', 'out'));
    const controller = new AbortController();
    let completedChunk = [];

    await assertAbortError(processBatchChunked(inputs, outputDir, {
      format: 'webp',
      quality: 80,
      chunkSize: 2,
      onProgress: (event) => {
        completedChunk = event.lastChunk;
        controller.abort();
      },
      signal: controller.signal,
    }));

    assert.equal(completedChunk.length, 2);
    for (const result of completedChunk) {
      assert(result.success);
      assert(result.outputPath);
      assert(fs.existsSync(result.outputPath), `completed output should remain: ${result.outputPath}`);
    }
  });

  await test('pre-aborted signal rejects before creating outputs', async () => {
    const inputs = makeInputs('pre-aborted', 3);
    const outputDir = resetDir(path.join(ROOT, 'pre-aborted', 'out'));
    const controller = new AbortController();
    controller.abort();

    await assertAbortError(processBatchChunked(inputs, outputDir, {
      format: 'webp',
      quality: 80,
      chunkSize: 2,
      signal: controller.signal,
    }));
    assert.deepEqual(fs.readdirSync(outputDir), []);
  });

  await test('continues when progress callback throws', async () => {
    const inputs = makeInputs('progress-throws', 4);
    const outputDir = resetDir(path.join(ROOT, 'progress-throws', 'out'));
    const originalError = console.error;
    const errors = [];
    console.error = (...args) => errors.push(args);
    try {
      const results = await processBatchChunked(inputs, outputDir, {
        format: 'jpeg',
        quality: 80,
        chunkSize: 2,
        onProgress: () => {
          throw new Error('progress failed');
        },
      });

      assert.equal(results.length, 4);
      assert(results.every((result) => result.success));
      assert.equal(errors.length, 2);
    } finally {
      console.error = originalError;
    }
  });

  await test('preserves processBatch partial-failure semantics', async () => {
    const inputs = makeInputs('partial-failure', 2);
    const missing = path.join(ROOT, 'partial-failure', 'missing.jpg');
    const outputDir = resetDir(path.join(ROOT, 'partial-failure', 'out'));

    const results = await processBatchChunked([inputs[0], missing, inputs[1]], outputDir, {
      format: 'webp',
      quality: 80,
      chunkSize: 2,
    });

    assert.equal(results.length, 3);
    assert.deepEqual(results.map((result) => result.source), [inputs[0], missing, inputs[1]]);
    const failed = results.find((result) => !result.success);
    assert(failed, 'one result should fail');
    assert.equal(failed.source, missing);
    assert.equal(typeof failed.errorCode, 'string');
  });

  await test('rejects invalid chunk sizes before returning a promise', async () => {
    const inputs = makeInputs('invalid-chunk-size', 1);
    const outputDir = resetDir(path.join(ROOT, 'invalid-chunk-size', 'out'));

    assert.throws(
      () => processBatchChunked(inputs, outputDir, { format: 'webp', chunkSize: 0 }),
      /chunkSize must be a positive integer/,
    );
    assert.throws(
      () => processBatchChunked(inputs, outputDir, { format: 'webp', chunkSize: 1.5 }),
      /chunkSize must be a positive integer/,
    );
  });
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}
