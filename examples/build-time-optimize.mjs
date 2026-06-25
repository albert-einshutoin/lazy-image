/**
 * Build-time static asset optimization example.
 *
 * Run:
 *   node examples/build-time-optimize.mjs <srcDir> <outDir>
 *   node examples/build-time-optimize.mjs --self-test
 *
 * With no arguments, the script runs the same fixture-backed self-test used by CI.
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

const { ImageEngine } = loadLazyImage();

const args = process.argv.slice(2).filter((arg) => arg !== '--self-test');

if (hasSelfTestFlag() || args.length < 2) {
  await runSelfTest();
} else {
  await optimizeDirectory(args[0], args[1], { exitOnFailure: true });
}

function listImageFiles(inputDir) {
  return fs.readdirSync(inputDir)
    .filter((file) => /\.(jpe?g|png|webp)$/i.test(file))
    .map((file) => path.join(inputDir, file));
}

async function optimizeDirectory(inputDir, outputDir, { exitOnFailure = false } = {}) {
  const inputs = listImageFiles(inputDir);
  if (inputs.length === 0) {
    throw new Error(`No jpg/png/webp files found in ${inputDir}`);
  }

  ensureDir(outputDir);

  const started = Date.now();
  const { items, summary } = await ImageEngine.fromPath(inputs[0])
    .resize({ width: 1600, fit: 'inside' })
    .processBatchWithMetrics(inputs, outputDir, {
      format: 'webp',
      quality: 80,
      concurrency: 0,
    });

  printSummary(summary, Date.now() - started);

  const failures = items.filter((item) => !item.success);
  for (const failure of failures) {
    console.error(`FAIL: ${failure.source} - ${failure.error} [${failure.errorCode}]`);
  }

  if (failures.length > 0 && exitOnFailure) {
    process.exitCode = 1;
  }

  return { items, summary, failures };
}

function printSummary(summary, elapsedMs) {
  const reductionPercent = summary.totalBytesIn > 0
    ? (1 - summary.totalBytesOut / summary.totalBytesIn) * 100
    : 0;

  console.log('Build-time optimization summary:');
  console.log(`  Files: ${summary.successfulItems}/${summary.totalItems} optimized`);
  console.log(`  Total in: ${(summary.totalBytesIn / 1024).toFixed(1)} KiB`);
  console.log(`  Total out: ${(summary.totalBytesOut / 1024).toFixed(1)} KiB`);
  console.log(`  Reduction: ${reductionPercent.toFixed(1)}%`);
  console.log(`  Native wall time: ${summary.totalWallMs.toFixed(0)} ms`);
  console.log(`  Script wall time: ${elapsedMs} ms`);
}

async function runSelfTest() {
  const tmp = makeTempDir('lazy-image-build-time-');

  try {
    const inputDir = ensureDir(path.join(tmp, 'input'));
    const outputDir = ensureDir(path.join(tmp, 'output'));

    copyFixture('test_100KB_1057x1057.jpg', inputDir, 'avatar.jpg');
    copyFixture('test_100KB_1188x1188.png', inputDir, 'hero.png');
    copyFixture('test_90KB_1471x1471.webp', inputDir, 'banner.webp');

    const { items, summary, failures } = await optimizeDirectory(inputDir, outputDir);
    assert.equal(items.length, 3);
    assert.equal(failures.length, 0);
    assert.equal(summary.successfulItems, 3);
    assert(summary.totalBytesIn > 0);
    assert(summary.totalBytesOut > 0);
  } finally {
    removeDir(tmp);
  }
}
