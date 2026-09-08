const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { ImageEngine } = require('../../index.js');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');

const INPUT_JPEG = resolveFixture('test_input.jpg');
const INPUT_PNG = resolveFixture('test_input.png');
const INPUT_EXIF_JPEG = resolveFixture('test_with_exif.jpg');
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

function requireOptionValue(args, index, optionName) {
  const value = args[index + 1];
  if (!value || value.startsWith('-')) {
    throw new Error(`${optionName} requires a path value`);
  }
  return value;
}

function resolveOutputPath(value) {
  if (!value) {
    return null;
  }
  return path.resolve(resolveRoot(), value);
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
    targetBytesRounds: process.env.NAPI_STRESS_TARGET_BYTES_ROUNDS
      ? parseNonNegativeInt(process.env.NAPI_STRESS_TARGET_BYTES_ROUNDS, 'NAPI_STRESS_TARGET_BYTES_ROUNDS')
      : null,
    summaryJsonPath: resolveOutputPath(process.env.NAPI_STRESS_SUMMARY_JSON),
    summaryMdPath: resolveOutputPath(process.env.NAPI_STRESS_SUMMARY_MD),
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
    } else if (arg === '--target-bytes-rounds') {
      options.targetBytesRounds = parseNonNegativeInt(args[i + 1], arg);
      i += 1;
    } else if (arg === '--summary-json') {
      options.summaryJsonPath = resolveOutputPath(requireOptionValue(args, i, arg));
      i += 1;
    } else if (arg === '--summary-md') {
      options.summaryMdPath = resolveOutputPath(requireOptionValue(args, i, arg));
      i += 1;
    } else if (!arg.startsWith('-')) {
      options.iterations = parsePositiveInt(arg, 'iterations');
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  if (options.targetBytesRounds == null) {
    options.targetBytesRounds = Math.min(options.iterations, 5);
  } else {
    options.targetBytesRounds = Math.min(options.targetBytesRounds, options.iterations);
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
        filePath,
        generatedFuzzSeed: true,
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

function roundMb(value) {
  return Number(value.toFixed(1));
}

function summarizeMalformedInputs(inputs) {
  const generatedFuzzTargets = {};
  let generatedFuzzSeeds = 0;
  let generatedFuzzFilePathSeeds = 0;

  for (const input of inputs) {
    if (!input.name.startsWith('fuzz/')) {
      continue;
    }

    generatedFuzzSeeds += 1;
    if (input.generatedFuzzSeed && input.filePath) {
      generatedFuzzFilePathSeeds += 1;
    }
    const target = input.name.split('/')[1] || 'unknown';
    generatedFuzzTargets[target] = (generatedFuzzTargets[target] || 0) + 1;
  }

  return {
    total: inputs.length,
    inline: inputs.length - generatedFuzzSeeds,
    generatedFuzzSeeds,
    generatedFuzzFilePathSeeds,
    generatedFuzzTargets,
  };
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

async function runTargetBytesRound(iteration, iterationDir) {
  const targetOptions = {
    targetBytes: 2500,
    minQuality: 35,
    maxQuality: 90,
  };

  const bufferResult = await ImageEngine.fromPath(INPUT_JPEG)
    .sanitize({ policy: 'strict' })
    .resize({ width: 320, fit: 'inside' })
    .toBufferTargetBytes('jpeg', targetOptions);

  if (!Buffer.isBuffer(bufferResult.data) || bufferResult.data.length === 0) {
    throw new Error(`iteration ${iteration}: target-bytes buffer search returned no bytes`);
  }
  if (bufferResult.bytesOut !== bufferResult.data.length) {
    throw new Error(`iteration ${iteration}: target-bytes buffer bytesOut mismatch`);
  }
  if (bufferResult.targetBytes !== targetOptions.targetBytes) {
    throw new Error(`iteration ${iteration}: target-bytes buffer target was not echoed`);
  }

  const targetFilePath = path.join(iterationDir, 'target-bytes-output.jpg');
  const fileResult = await ImageEngine.fromPath(INPUT_JPEG)
    .sanitize({ policy: 'strict' })
    .resize({ width: 320, fit: 'inside' })
    .toFileTargetBytes(targetFilePath, 'jpeg', targetOptions);

  if (!fileResult || fileResult.bytesWritten <= 0 || !fs.existsSync(targetFilePath)) {
    throw new Error(`iteration ${iteration}: target-bytes file search wrote no bytes`);
  }
  if (fileResult.targetBytes !== targetOptions.targetBytes) {
    throw new Error(`iteration ${iteration}: target-bytes file target was not echoed`);
  }
}

async function runIteration(iteration, rootDir, includeTargetBytesRound) {
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

  const cloneEngine = ImageEngine.fromPath(INPUT_EXIF_JPEG)
    .keepMetadata({ icc: true, exif: true, stripGps: false })
    .autoOrient(false)
    .sanitize({ policy: 'public-upload' })
    .resize({ width: 160, fit: 'inside' });
  const [cloneJpeg, cloneWebp, cloneMetrics] = await Promise.all([
    cloneEngine.clone().toBuffer('jpeg', 76),
    cloneEngine.clone().toBuffer('webp', 76),
    cloneEngine.clone().toBufferWithMetrics('jpeg', 72),
  ]);

  if (!Buffer.isBuffer(cloneJpeg) || cloneJpeg.length === 0) {
    throw new Error(`iteration ${iteration}: clone JPEG output returned no bytes`);
  }
  if (cloneJpeg.includes(Buffer.from('Exif\0\0'))) {
    throw new Error(`iteration ${iteration}: public-upload clone retained EXIF`);
  }
  if (!Buffer.isBuffer(cloneWebp) || cloneWebp.length === 0) {
    throw new Error(`iteration ${iteration}: clone WebP output returned no bytes`);
  }
  if (!Buffer.isBuffer(cloneMetrics.data) || cloneMetrics.data.length === 0) {
    throw new Error(`iteration ${iteration}: clone metrics output returned no bytes`);
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

  if (includeTargetBytesRound) {
    await runTargetBytesRound(iteration, iterationDir);
  }
}

async function runMalformedRound(round, malformedInputs) {
  for (const input of malformedInputs) {
    await expectRejects(`malformed round ${round}: ${input.name}`, () => (
      ImageEngine.from(input.data)
        .sanitize({ policy: 'public-upload' })
        .resize({ width: 64, fit: 'inside' })
        .toBufferWithMetrics('jpeg', 80)
    ));
  }

  for (const input of malformedInputs) {
    if (!input.generatedFuzzSeed || !input.filePath) {
      continue;
    }

    await expectRejects(`malformed round ${round}: file ${input.name}`, () => (
      ImageEngine.fromPath(input.filePath)
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

function summarizeMemorySamples(samples) {
  const measured = samples.slice(1);
  const peakSamples = measured.length > 0 ? measured : samples;
  const baseline = measured[0]?.rssMb || samples[0]?.rssMb || 0;
  const peak = Math.max(...peakSamples.map((sample) => sample.rssMb), 0);
  const growth = Math.max(0, peak - baseline);

  return {
    sampleCount: samples.length,
    baselineRssMb: roundMb(baseline),
    peakRssMb: roundMb(peak),
    growthRssMb: roundMb(growth),
    samples: samples.map((sample) => ({
      phase: sample.phase,
      rssMb: roundMb(sample.rssMb),
    })),
  };
}

function assertRssGrowthWithinLimit(memorySummary, limitMb) {
  if (limitMb <= 0 || memorySummary.sampleCount < 3) {
    return;
  }

  if (memorySummary.growthRssMb > limitMb) {
    throw new Error(
      `RSS growth ${memorySummary.growthRssMb.toFixed(1)} MiB exceeded limit ${limitMb} MiB ` +
        `(baseline ${memorySummary.baselineRssMb.toFixed(1)} MiB, ` +
        `peak ${memorySummary.peakRssMb.toFixed(1)} MiB)`,
    );
  }
}

function buildSummary(options, malformedInputs, memorySamples) {
  return {
    version: 1,
    tool: 'napi-boundary-stress',
    generatedAt: new Date().toISOString(),
    status: 'passed',
    options: {
      iterations: options.iterations,
      malformedRounds: options.malformedRounds,
      targetBytesRounds: options.targetBytesRounds,
      rssGrowthLimitMb: options.rssGrowthLimitMb,
    },
    boundaryCoverage: {
      cloneMultiOutputRounds: options.iterations,
      publicUploadCloneRounds: options.iterations,
      publicUploadMalformedBufferAttempts: options.malformedRounds * malformedInputs.length,
      targetBytesSearchRounds: options.targetBytesRounds,
      targetBytesSearches: options.targetBytesRounds * 2,
    },
    malformedInputs: summarizeMalformedInputs(malformedInputs),
    memory: summarizeMemorySamples(memorySamples),
  };
}

function formatSummaryMarkdown(summary) {
  const targetEntries = Object.entries(summary.malformedInputs.generatedFuzzTargets);
  const targetRows = targetEntries.length === 0
    ? ['_No generated fuzz seed targets._']
    : [
        '| target | seeds |',
        '| --- | ---: |',
        ...targetEntries.map(([target, count]) => `| ${target} | ${count} |`),
      ];

  return [
    '# NAPI Boundary Stress Summary',
    '',
    `- status: ${summary.status}`,
    `- generatedAt: ${summary.generatedAt}`,
    `- iterations: ${summary.options.iterations}`,
    `- malformedRounds: ${summary.options.malformedRounds}`,
    `- targetBytesRounds: ${summary.options.targetBytesRounds}`,
    `- cloneMultiOutputRounds: ${summary.boundaryCoverage.cloneMultiOutputRounds}`,
    `- publicUploadCloneRounds: ${summary.boundaryCoverage.publicUploadCloneRounds}`,
    `- publicUploadMalformedBufferAttempts: ${summary.boundaryCoverage.publicUploadMalformedBufferAttempts}`,
    `- targetBytesSearches: ${summary.boundaryCoverage.targetBytesSearches}`,
    `- malformedInputs: ${summary.malformedInputs.total}`,
    `- generatedFuzzSeeds: ${summary.malformedInputs.generatedFuzzSeeds}`,
    `- generatedFuzzFilePathSeeds: ${summary.malformedInputs.generatedFuzzFilePathSeeds}`,
    `- peakRss: ${summary.memory.peakRssMb.toFixed(1)} MiB`,
    `- rssGrowth: ${summary.memory.growthRssMb.toFixed(1)} MiB`,
    `- rssGrowthLimit: ${summary.options.rssGrowthLimitMb.toFixed(1)} MiB`,
    '',
    '## Generated Fuzz Seed Targets',
    '',
    ...targetRows,
    '',
  ].join('\n');
}

function writeSummaryArtifacts(summary, options) {
  const writes = [
    [options.summaryJsonPath, `${JSON.stringify(summary, null, 2)}\n`],
    [options.summaryMdPath, formatSummaryMarkdown(summary)],
  ];

  for (const [filePath, contents] of writes) {
    if (!filePath) {
      continue;
    }

    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, contents);
    console.error(`napi stress summary written: ${filePath}`);
  }
}

async function main() {
  const options = parseOptions();
  const memorySamples = [];
  const rootDir = resolveTemp(`napi-boundary-stress-${process.pid}`);
  fs.rmSync(rootDir, { recursive: true, force: true });
  fs.mkdirSync(rootDir, { recursive: true });
  const malformedInputs = buildMalformedInputs(rootDir);

  recordMemorySample(memorySamples, 'start');

  for (let i = 0; i < options.iterations; i += 1) {
    await runIteration(i, rootDir, i < options.targetBytesRounds);
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

  const summary = buildSummary(options, malformedInputs, memorySamples);
  try {
    assertRssGrowthWithinLimit(summary.memory, options.rssGrowthLimitMb);
  } catch (error) {
    summary.status = 'failed';
    summary.error = { message: error.message };
    writeSummaryArtifacts(summary, options);
    throw error;
  }
  writeSummaryArtifacts(summary, options);

  fs.rmSync(rootDir, { recursive: true, force: true });
  console.error(
    `napi stress completed: iterations=${options.iterations}, ` +
      `malformedRounds=${options.malformedRounds}, targetBytesRounds=${options.targetBytesRounds}, ` +
      `malformedInputs=${malformedInputs.length}, ` +
      `peakRss=${summary.memory.peakRssMb.toFixed(1)} MiB`,
  );
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
