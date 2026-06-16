/**
 * Batch processing examples for lazy-image.
 *
 * Run:
 *   node examples/batch-processing.mjs
 *   node examples/batch-processing.mjs --self-test
 */

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  copyFixture,
  ensureDir,
  hasSelfTestFlag,
  loadLazyImage,
  makeTempDir,
  removeDir,
} from './_load.mjs';

const { ImageEngine, processBatchChunked } = loadLazyImage();

if (hasSelfTestFlag()) {
  await runSelfTest();
} else {
  await runDemo('./images', './optimized');
}

function listImageFiles(inputDir) {
  return fs.readdirSync(inputDir)
    .filter((file) => /\.(jpe?g|png|webp)$/i.test(file))
    .map((file) => path.join(inputDir, file));
}

function assertAllSucceeded(results) {
  const failures = results.filter((result) => !result.success);
  for (const failure of failures) {
    console.error(`FAIL: ${failure.source} - ${failure.error} [${failure.errorCode}]`);
  }
  assert.equal(failures.length, 0, 'all batch items should succeed');
}

async function runSelfTest() {
  const tmp = makeTempDir('lazy-image-batch-');

  try {
    const inputDir = ensureDir(path.join(tmp, 'input'));
    const simpleOutputDir = ensureDir(path.join(tmp, 'simple-output'));
    const metricsOutputDir = ensureDir(path.join(tmp, 'metrics-output'));
    const chunkedOutputDir = ensureDir(path.join(tmp, 'chunked-output'));

    copyFixture('test_100KB_1057x1057.jpg', inputDir, 'avatar.jpg');
    copyFixture('test_100KB_1188x1188.png', inputDir, 'hero.png');
    copyFixture('test_90KB_1471x1471.webp', inputDir, 'banner.webp');

    const files = listImageFiles(inputDir);
    assert.equal(files.length, 3);

    const results = await ImageEngine.fromPath(files[0])
      .resize(1200)
      .processBatch(files, simpleOutputDir, {
        format: 'webp',
        quality: 80,
        concurrency: 1,
      });
    assert.equal(results.length, files.length);
    assertAllSucceeded(results);

    const { items, summary } = await ImageEngine.fromPath(files[0])
      .resize(800)
      .processBatchWithMetrics(files, metricsOutputDir, {
        format: 'jpeg',
        quality: 85,
        concurrency: 1,
      });

    assert.equal(items.length, files.length);
    assert.equal(summary.totalItems, files.length);
    assert.equal(summary.successfulItems, files.length);
    assert(summary.totalBytesIn > 0);
    assert(summary.totalBytesOut > 0);
    assertAllSucceeded(items);

    const progressEvents = [];
    const chunkedResults = await processBatchChunked(files, chunkedOutputDir, {
      format: 'webp',
      quality: 80,
      concurrency: 1,
      chunkSize: 2,
      onProgress: (event) => progressEvents.push(event),
    });

    assert.equal(chunkedResults.length, files.length);
    assertAllSucceeded(chunkedResults);
    assert.equal(progressEvents.length, 2);
    assert.equal(progressEvents.at(-1).completed, files.length);
  } finally {
    removeDir(tmp);
  }
}

async function runDemo(inputDir, outputDir) {
  const files = listImageFiles(inputDir);
  if (files.length === 0) {
    throw new Error(`No jpg/png/webp files found in ${inputDir}`);
  }

  // 1. Simple batch with auto concurrency
  const results = await ImageEngine.fromPath(files[0])
    .resize(1200)
    .processBatch(files, outputDir, {
      format: 'webp',
      quality: 80,
      // concurrency: 0 = auto-detect based on CPU cores and memory
    });

  for (const result of results) {
    if (result.success) {
      console.log(`OK: ${result.source} -> ${result.outputPath}`);
    } else {
      console.error(`FAIL: ${result.source} - ${result.error} [${result.errorCode}]`);
    }
  }

  // 2. Batch with metrics and manual concurrency
  const { summary } = await ImageEngine.fromPath(files[0])
    .resize(800)
    .processBatchWithMetrics(files, outputDir, {
      format: 'jpeg',
      quality: 85,
      concurrency: 4,
    });

  console.log('\nBatch summary:');
  console.log(`  Success: ${summary.successfulItems}/${summary.totalItems}`);
  console.log(`  Concurrency: ${summary.effectiveConcurrency} (auto: ${summary.autoConcurrency})`);
  console.log(`  Total in: ${(summary.totalBytesIn / 1024).toFixed(0)} KB`);
  console.log(`  Total out: ${(summary.totalBytesOut / 1024).toFixed(0)} KB`);
  console.log(`  Wall time: ${summary.totalWallMs.toFixed(0)} ms`);

  // 3. Chunked batch with progress and AbortSignal
  const controller = new AbortController();
  const chunkedResults = await processBatchChunked(files, outputDir, {
    format: 'webp',
    quality: 80,
    concurrency: 4,
    chunkSize: 16,
    signal: controller.signal,
    onProgress: ({ completed, total, failed }) => {
      const percent = Math.round((completed / total) * 100);
      process.stdout.write(`\r${completed}/${total} (${percent}%) complete, ${failed} failed`);
    },
  });

  process.stdout.write('\n');
  console.log(`Chunked batch complete: ${chunkedResults.length} results`);
}
