const fs = require('node:fs');
const path = require('node:path');
const { ImageEngine } = require('../../index.js');
const { resolveFixture, resolveTemp } = require('../helpers/paths');

const INPUT_JPEG = resolveFixture('test_input.jpg');
const INPUT_PNG = resolveFixture('test_input.png');

function parseIterations() {
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--iterations' || arg === '-n') {
      const parsed = Number.parseInt(args[i + 1] || '', 10);
      if (Number.isFinite(parsed) && parsed > 0) {
        return parsed;
      }
    } else {
      const parsed = Number.parseInt(arg, 10);
      if (Number.isFinite(parsed) && parsed > 0) {
        return parsed;
      }
    }
  }
  return 20;
}

async function runIteration(iteration, rootDir) {
  const iterationDir = path.join(rootDir, `iter-${iteration}`);
  const batchDir = path.join(iterationDir, 'batch');
  fs.mkdirSync(batchDir, { recursive: true });

  const jpegBytes = await ImageEngine.fromPath(INPUT_JPEG)
    .sanitize({ policy: 'strict' })
    .resize({ width: 1200, fit: 'inside' })
    .toBuffer('jpeg', 82);

  if (!Buffer.isBuffer(jpegBytes) || jpegBytes.length === 0) {
    throw new Error(`iteration ${iteration}: jpeg encode returned no bytes`);
  }

  const metricsResult = await ImageEngine.from(jpegBytes)
    .resize({ width: 640, height: 640, fit: 'cover' })
    .toBufferWithMetrics('webp', 78);

  if (!Buffer.isBuffer(metricsResult.data) || metricsResult.data.length === 0) {
    throw new Error(`iteration ${iteration}: metrics encode returned no bytes`);
  }

  const fileResult = await ImageEngine.fromPath(INPUT_PNG)
    .resize({ width: 512, fit: 'inside' })
    .toFileWithMetrics(path.join(iterationDir, 'single-output.webp'), 'webp', 80);

  if (!fileResult || fileResult.bytesWritten <= 0) {
    throw new Error(`iteration ${iteration}: file output wrote no bytes`);
  }

  const batchResult = await ImageEngine.fromPath(INPUT_JPEG)
    .resize({ width: 256, fit: 'inside' })
    .processBatchWithMetrics(
      [INPUT_JPEG, INPUT_PNG],
      batchDir,
      { format: 'jpeg', quality: 75, concurrency: 2 },
    );

  if (!batchResult || batchResult.items.length !== 2) {
    throw new Error(`iteration ${iteration}: unexpected batch item count`);
  }

  const failed = batchResult.items.find((item) => !item.success);
  if (failed) {
    throw new Error(`iteration ${iteration}: batch failed for ${failed.source}: ${failed.error}`);
  }
}

async function main() {
  const iterations = parseIterations();
  const rootDir = resolveTemp('napi-boundary-stress');
  fs.rmSync(rootDir, { recursive: true, force: true });
  fs.mkdirSync(rootDir, { recursive: true });

  for (let i = 0; i < iterations; i += 1) {
    await runIteration(i, rootDir);
    if (i % 5 === 0) {
      console.error(`napi stress iteration ${i} completed`);
    }
  }

  fs.rmSync(rootDir, { recursive: true, force: true });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
