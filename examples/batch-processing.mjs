/**
 * Batch processing examples for lazy-image.
 *
 * Run:
 *   node examples/batch-processing.mjs
 */

import { ImageEngine, processBatchChunked } from '@alberteinshutoin/lazy-image';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const inputDir = './images';
const outputDir = './optimized';

// ── 1. Simple batch with auto concurrency ──────────────────────────
const files = readdirSync(inputDir)
  .filter((f) => /\.(jpe?g|png|webp)$/i.test(f))
  .map((f) => join(inputDir, f));

const results = await ImageEngine.fromPath(files[0])
  .resize(1200)
  .processBatch(files, outputDir, {
    format: 'webp',
    quality: 80,
    // concurrency: 0 = auto-detect based on CPU cores and memory
  });

for (const r of results) {
  if (r.success) {
    console.log(`OK: ${r.source} -> ${r.outputPath}`);
  } else {
    console.error(`FAIL: ${r.source} — ${r.error} [${r.errorCode}]`);
  }
}

// ── 2. Batch with metrics and manual concurrency ───────────────────
const { items, summary } = await ImageEngine.fromPath(files[0])
  .resize(800)
  .processBatchWithMetrics(files, outputDir, {
    format: 'jpeg',
    quality: 85,
    concurrency: 4,
  });

console.log(`\nBatch summary:`);
console.log(`  Success: ${summary.successfulItems}/${summary.totalItems}`);
console.log(`  Concurrency: ${summary.effectiveConcurrency} (auto: ${summary.autoConcurrency})`);
console.log(`  Total in: ${(summary.totalBytesIn / 1024).toFixed(0)} KB`);
console.log(`  Total out: ${(summary.totalBytesOut / 1024).toFixed(0)} KB`);
console.log(`  Wall time: ${summary.totalWallMs.toFixed(0)} ms`);

// ── 3. Chunked batch with progress and AbortSignal ─────────────────
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
