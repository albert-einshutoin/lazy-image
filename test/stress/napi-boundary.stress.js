const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { ImageEngine } = require('../../index.js');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');

const INPUT_JPEG = resolveFixture('test_input.jpg');
const INPUT_PNG = resolveFixture('test_input.png');
const FUZZ_SEED_SCRIPT = resolveRoot('scripts/fuzz/write-seed-corpus.mjs');

function parsePositiveInt(value, optionName) {
  const parsed = Number.parseInt(value || '', 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return parsed;
  }
  throw new Error(`${optionName} must be a positive integer`);
}

function parseNonNegativeNumber(value, optionName) {
  const parsed = Number.parseFloat(value || '');
  if (Number.isFinite(parsed) && parsed >= 0) {
    return parsed;
  }
  throw new Error(`${optionName} must be a non-negative number`);
}

function parseNonNegativeInt(value, optionName) {
  const parsed = Number.parseInt(value || '', 10);
  if (Number.isFinite(parsed) && parsed >= 0) {
    return parsed;
  }
  throw new Error(`${optionName} must be a non-negative integer`);
}

function parseOptions() {
  const args = process.argv.slice(2);
  const options = {
    iterations: process.env.NAPI_STRESS_ITERATIONS
      ? parsePositiveInt(process.env.NAPI_STRESS_ITERATIONS, 'NAPI_STRESS_ITERATIONS')
      : 20,
    malformedRounds: process.env.NAPI_STRESS_MALFORMED_ROUNDS
      ? parseNonNegativeInt(process.env.NAPI_STRESS_MALFORMED_ROUNDS, 'NAPI_STRESS_MALFORMED_ROUNDS')
      : 20,
    rssGrowthLimitMb: process.env.NAPI_STRESS_RSS_GROWTH_LIMIT_MB
      ? parseNonNegativeNumber(process.env.NAPI_STRESS_RSS_GROWTH_LIMIT_MB, 'NAPI_STRESS_RSS_GROWTH_LIMIT_MB')
      : 0,
  };

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--iterations' || arg === '-n') {
      options.iterations = parsePositiveInt(args[i + 1], arg);
      i += 1;
    } else if (arg === '--malformed-rounds') {
      options.malformedRounds = parseNonNegativeInt(args[i + 1], arg);
      i += 1;
    } else if (arg === '--rss-growth-limit-mb') {
      options.rssGrowthLimitMb = parseNonNegativeNumber(args[i + 1], arg);
      i += 1;
    } else if (!arg.startsWith('-')) {
      options.iterations = parsePositiveInt(arg, 'iterations');
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

function buildInlineMalformedInputs() {
  const jpeg = fs.readFileSync(INPUT_JPEG);
  return [
    { name: 'empty', data: Buffer.alloc(0) },
    { name: 'random-bytes', data: Buffer.from([0x42, 0x00, 0xff, 0x13, 0x7f, 0x80, 0x01, 0x02]) },
    { name: 'jpeg-soi-only', data: Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]) },
    { name: 'truncated-jpeg', data: jpeg.subarray(0, Math.min(jpeg.length, 200)) },
    {
      name: 'png-huge-dimensions-header',
      data: Buffer.from([
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
        0x00, 0x00, 0x00, 0x0d,
        0x49, 0x48, 0x44, 0x52,
        0x7f, 0xff, 0xff, 0xff,
        0x7f, 0xff, 0xff, 0xff,
        0x08, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
      ]),
    },
  ];
}

function collectGeneratedFuzzSeeds(corpusRoot) {
  const result = spawnSync(process.execPath, [
    FUZZ_SEED_SCRIPT,
    '--corpus-root',
    corpusRoot,
  ], {
    cwd: resolveRoot(),
    encoding: 'utf8',
  });

  if (result.status !== 0) {
    throw new Error(
      `failed to generate malformed fuzz seed corpus: ${result.stderr || result.stdout}`,
    );
  }

  const inputs = [];
  for (const target of fs.readdirSync(corpusRoot).sort()) {
    const targetDir = path.join(corpusRoot, target);
    if (!fs.statSync(targetDir).isDirectory()) {
      continue;
    }

    for (const fileName of fs.readdirSync(targetDir).sort()) {
      const filePath = path.join(targetDir, fileName);
      if (!fs.statSync(filePath).isFile()) {
        continue;
      }

      inputs.push({
        name: `fuzz/${target}/${fileName}`,
        data: fs.readFileSync(filePath),
      });
    }
  }

  if (inputs.length === 0) {
    throw new Error('malformed fuzz seed corpus generation produced no inputs');
  }

  return inputs;
}

function buildMalformedInputs(rootDir) {
  const corpusRoot = path.join(rootDir, 'malformed-fuzz-corpus');
  fs.rmSync(corpusRoot, { recursive: true, force: true });
  return [
    ...buildInlineMalformedInputs(),
    ...collectGeneratedFuzzSeeds(corpusRoot),
  ];
}

async function expectRejects(label, fn) {
  try {
    await fn();
  } catch (error) {
    if (!error || typeof error.message !== 'string' || error.message.length === 0) {
      throw new Error(`${label}: rejection did not expose a useful Error`);
    }
    if (typeof error.errorCode !== 'string' || error.errorCode.length === 0) {
      throw new Error(`${label}: rejection did not expose a structured lazy-image errorCode`);
    }
    return;
  }

  throw new Error(`${label}: unexpectedly succeeded`);
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

async function runMalformedRound(round, malformedInputs) {
  for (const input of malformedInputs) {
    await expectRejects(`malformed round ${round}: ${input.name}`, () => (
      ImageEngine.from(input.data)
        .sanitize({ policy: 'strict' })
        .resize({ width: 64, fit: 'inside' })
        .toBufferWithMetrics('jpeg', 80)
    ));
  }

  await expectRejects(`malformed round ${round}: unsupported format`, () => (
    ImageEngine.fromPath(INPUT_JPEG)
      .resize({ width: 64, fit: 'inside' })
      .toBuffer('gif', 80)
  ));

  await expectRejects(`malformed round ${round}: invalid sanitize policy`, () => (
    ImageEngine.fromPath(INPUT_JPEG).sanitize({ policy: 'invalid_policy' })
  ));

  await expectRejects(`malformed round ${round}: invalid rotation`, () => (
    ImageEngine.fromPath(INPUT_JPEG)
      .rotate(45)
      .toBuffer('jpeg', 80)
  ));
}

function maybeCollectGarbage() {
  if (typeof global.gc === 'function') {
    global.gc();
  }
}

function recordMemorySample(samples, phase) {
  maybeCollectGarbage();
  samples.push({
    phase,
    rssMb: process.memoryUsage().rss / 1024 / 1024,
  });
}

function assertRssGrowthWithinLimit(samples, limitMb) {
  if (limitMb <= 0 || samples.length < 3) {
    return;
  }

  const measured = samples.slice(1);
  const baseline = measured[0].rssMb;
  const peak = Math.max(...measured.map((sample) => sample.rssMb));
  const growth = peak - baseline;

  if (growth > limitMb) {
    throw new Error(
      `RSS growth ${growth.toFixed(1)} MiB exceeded limit ${limitMb} MiB ` +
        `(baseline ${baseline.toFixed(1)} MiB, peak ${peak.toFixed(1)} MiB)`,
    );
  }
}

async function main() {
  const options = parseOptions();
  const memorySamples = [];
  const rootDir = resolveTemp('napi-boundary-stress');
  fs.rmSync(rootDir, { recursive: true, force: true });
  fs.mkdirSync(rootDir, { recursive: true });
  const malformedInputs = buildMalformedInputs(rootDir);

  recordMemorySample(memorySamples, 'start');

  for (let i = 0; i < options.iterations; i += 1) {
    await runIteration(i, rootDir);
    if (i < options.malformedRounds) {
      await runMalformedRound(i, malformedInputs);
    }
    recordMemorySample(memorySamples, `iteration-${i}`);
    if (i % 5 === 0) {
      console.error(`napi stress iteration ${i} completed`);
    }
  }

  for (let i = options.iterations; i < options.malformedRounds; i += 1) {
    await runMalformedRound(i, malformedInputs);
    recordMemorySample(memorySamples, `malformed-${i}`);
  }

  assertRssGrowthWithinLimit(memorySamples, options.rssGrowthLimitMb);

  fs.rmSync(rootDir, { recursive: true, force: true });
  const peakRssMb = Math.max(...memorySamples.map((sample) => sample.rssMb));
  console.error(
    `napi stress completed: iterations=${options.iterations}, ` +
      `malformedRounds=${options.malformedRounds}, malformedInputs=${malformedInputs.length}, ` +
      `peakRss=${peakRssMb.toFixed(1)} MiB`,
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
