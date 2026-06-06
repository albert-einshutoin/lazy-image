const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const sharp = require('sharp');
const { resolveRoot, resolveTemp } = require('../helpers/paths');
const { calculateQualityMetrics } = require('../helpers/quality');
const { buildCorpusManifestMarkdown, describeReferenceCorpusEntry } = require('../helpers/benchmark-corpus');
const {
  JPEG_BACKEND_BAKEOFF_CASES: CASES,
  buildBakeoffCorpusExpectations,
} = require('../helpers/codec-bakeoff-cases');
const {
  DEFAULT_JPEGLI_DISTANCE_CANDIDATES,
  buildDistanceCandidates,
  buildTuningCandidate,
  chooseMatchedBytesCandidate,
  chooseMatchedQualityCandidate,
  parseDistanceCandidateList,
} = require('../helpers/jpegli-distance-tuning');
const { buildPackageImpactMarkdown, collectBenchmarkPackageImpact } = require('../helpers/package-impact');
const { ImageEngine } = require(resolveRoot('index'));

const DEFAULT_ITERATIONS = 3;
const OUTPUT_FILE_BASE = 'jpegli-bakeoff';
const TEMP_DIR = resolveTemp('benchmarks', OUTPUT_FILE_BASE);

function getArg(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1) return fallback;
  const value = process.argv[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function hasFlag(name) {
  return process.argv.includes(name);
}

function parsePositiveInteger(value, name) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

function average(values) {
  return values.reduce((sum, value) => sum + value, 0) / Math.max(values.length, 1);
}

function formatNumber(value, digits = 3) {
  if (!Number.isFinite(value)) return 'inf';
  return value.toFixed(digits);
}

function formatPercent(value) {
  return `${formatNumber(value, 1)}%`;
}

function readPackageVersion() {
  const pkg = JSON.parse(fs.readFileSync(resolveRoot('package.json'), 'utf8'));
  return pkg.version;
}

function readCargoLockVersion(crateName) {
  const lockPath = resolveRoot('Cargo.lock');
  const lock = fs.readFileSync(lockPath, 'utf8');
  const packageBlocks = lock.split('\n[[package]]\n');

  for (const block of packageBlocks) {
    const nameMatch = block.match(/^name = "([^"]+)"/m);
    const versionMatch = block.match(/^version = "([^"]+)"/m);
    if (nameMatch?.[1] === crateName && versionMatch?.[1]) {
      return versionMatch[1];
    }
  }

  return null;
}

function firstLine(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean) || null;
}

function clampByte(value) {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function buildRgbGradient(width, height) {
  const channels = 3;
  const data = Buffer.alloc(width * height * channels);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * channels;
      data[offset] = clampByte((x / Math.max(width - 1, 1)) * 255);
      data[offset + 1] = clampByte((y / Math.max(height - 1, 1)) * 255);
      data[offset + 2] = clampByte(((x + y) / Math.max(width + height - 2, 1)) * 255);
    }
  }
  return data;
}

function buildUiScreenshot(width, height) {
  const channels = 3;
  const data = Buffer.alloc(width * height * channels);
  const topBar = Math.max(36, Math.round(height * 0.12));
  const sidebar = Math.max(92, Math.round(width * 0.18));

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * channels;
      let r = 246;
      let g = 248;
      let b = 250;

      if (y < topBar) {
        r = 31;
        g = 41;
        b = 55;
      } else if (x < sidebar) {
        r = 229;
        g = 233;
        b = 240;
      } else if ((x - sidebar) % 96 < 72 && (y - topBar) % 82 < 54) {
        r = 255;
        g = 255;
        b = 255;
      }

      if ((x + y) % 41 === 0 || (x > sidebar && y > topBar && (x - y) % 137 === 0)) {
        r = 59;
        g = 130;
        b = 246;
      }

      if (x % 128 === 0 || y % 96 === 0) {
        r = Math.max(0, r - 16);
        g = Math.max(0, g - 16);
        b = Math.max(0, b - 16);
      }

      data[offset] = r;
      data[offset + 1] = g;
      data[offset + 2] = b;
    }
  }
  return data;
}

function buildHighDetailTexture(width, height) {
  const channels = 3;
  const data = Buffer.alloc(width * height * channels);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * channels;
      const wave = Math.sin(x * 0.19) * 38 + Math.cos(y * 0.23) * 31;
      const grain = ((x * 1103515245 + y * 12345 + (x ^ y) * 2654435761) >>> 0) & 0xff;
      const base = 108 + wave + (grain - 128) * 0.42;
      data[offset] = clampByte(base + ((x & 7) === 0 ? 28 : -8));
      data[offset + 1] = clampByte(base + ((y & 7) === 0 ? 22 : 4));
      data[offset + 2] = clampByte(base + (((x + y) & 15) === 0 ? 34 : -12));
    }
  }
  return data;
}

function probeCjpegli(cjpegli, allowMissing) {
  const result = spawnSync(cjpegli, ['--help'], { encoding: 'utf8' });

  if (result.error) {
    if (result.error.code === 'ENOENT' && allowMissing) {
      return {
        available: false,
        reason: `${cjpegli} was not found`,
      };
    }

    throw new Error(
      [
        `Unable to execute cjpegli binary: ${cjpegli}`,
        `error: ${result.error.message}`,
        'Install jpegli and retry, or set JPEGLI_CJPEGLI=/path/to/cjpegli.',
      ].join('\n')
    );
  }

  const versionResult = spawnSync(cjpegli, ['--version'], { encoding: 'utf8' });
  const version = versionResult.error || versionResult.status !== 0
    ? null
    : firstLine(versionResult.stdout || versionResult.stderr || '');

  return {
    available: true,
    help: firstLine(result.stdout || result.stderr || ''),
    version,
  };
}

async function buildReferencePng(testCase) {
  if (testCase.source === 'generated-smooth-gradient') {
    const data = buildRgbGradient(testCase.width, testCase.height);
    return sharp(data, {
      raw: { width: testCase.width, height: testCase.height, channels: 3 },
    })
      .png({ compressionLevel: 0 })
      .toBuffer();
  }

  if (testCase.source === 'generated-ui-screenshot') {
    const data = buildUiScreenshot(testCase.width, testCase.height);
    return sharp(data, {
      raw: { width: testCase.width, height: testCase.height, channels: 3 },
    })
      .png({ compressionLevel: 0 })
      .toBuffer();
  }

  if (testCase.source === 'generated-high-detail-texture') {
    const data = buildHighDetailTexture(testCase.width, testCase.height);
    return sharp(data, {
      raw: { width: testCase.width, height: testCase.height, channels: 3 },
    })
      .png({ compressionLevel: 0 })
      .toBuffer();
  }

  return sharp(testCase.input)
    .resize(testCase.width, null, { withoutEnlargement: true })
    .png({ compressionLevel: 0 })
    .toBuffer();
}

function inputLabel(testCase) {
  if (testCase.input) return path.relative(resolveRoot(), testCase.input);
  return testCase.source || testCase.label;
}

async function encodeMozjpeg(referencePng, quality) {
  const start = process.hrtime.bigint();
  const data = await ImageEngine.from(referencePng).toBuffer('jpeg', quality);
  const encodeMs = Number(process.hrtime.bigint() - start) / 1_000_000;
  return { data, encodeMs };
}

function runCjpegli(cjpegli, inputPath, outputPath, distance) {
  const args = [inputPath, outputPath, '--distance', String(distance), '--quiet'];
  const start = process.hrtime.bigint();
  const result = spawnSync(cjpegli, args, { encoding: 'utf8' });
  const encodeMs = Number(process.hrtime.bigint() - start) / 1_000_000;

  if (result.error || result.status !== 0) {
    const details = [
      `cjpegli failed: ${cjpegli} ${args.join(' ')}`,
      result.error ? `error: ${result.error.message}` : null,
      result.status !== null ? `exit status: ${result.status}` : null,
      result.stdout ? `stdout:\n${result.stdout}` : null,
      result.stderr ? `stderr:\n${result.stderr}` : null,
    ].filter(Boolean);
    throw new Error(details.join('\n'));
  }

  return { data: fs.readFileSync(outputPath), encodeMs };
}

async function runDistanceTuning({
  testCase,
  options,
  referencePng,
  pngPath,
  mozjpegData,
  mozjpegQuality,
}) {
  if (!options.tuneDistances) {
    return {
      enabled: false,
      reason: 'disabled by --skip-distance-tuning or JPEGLI_SKIP_DISTANCE_TUNING=1',
    };
  }

  const distanceCandidates = buildDistanceCandidates(testCase.jpegliDistance, options.distanceCandidates);
  const candidates = [];

  for (const distance of distanceCandidates) {
    const outputPath = path.join(TEMP_DIR, `${testCase.label}-distance-${distance}.jpegli.jpg`);
    const output = path.relative(resolveRoot(), outputPath);
    const command = [
      options.cjpegli,
      path.relative(resolveRoot(), pngPath),
      output,
      '--distance',
      String(distance),
      '--quiet',
    ].join(' ');
    const encoded = runCjpegli(options.cjpegli, pngPath, outputPath, distance);
    const quality = await calculateQualityMetrics(encoded.data, referencePng);

    candidates.push({
      ...buildTuningCandidate({
        distance,
        bytes: encoded.data.length,
        encodeMs: encoded.encodeMs,
        ssim: quality.ssim,
        psnr: quality.psnr,
        targetBytes: mozjpegData.length,
        targetSsim: mozjpegQuality.ssim,
        targetPsnr: mozjpegQuality.psnr,
      }),
      output,
      command,
    });
  }

  return {
    enabled: true,
    target: {
      mozjpegBytes: mozjpegData.length,
      mozjpegSsim: mozjpegQuality.ssim,
      mozjpegPsnr: mozjpegQuality.psnr,
    },
    distanceCandidates,
    candidates,
    matchedBytes: chooseMatchedBytesCandidate(candidates),
    matchedQuality: chooseMatchedQualityCandidate(candidates),
  };
}

async function runCase(testCase, options) {
  const referencePng = await buildReferencePng(testCase);
  const pngPath = path.join(TEMP_DIR, `${testCase.label}.png`);
  fs.writeFileSync(pngPath, referencePng);
  const corpusEntry = await describeReferenceCorpusEntry({
    label: testCase.label,
    sourcePath: testCase.input || null,
    sourceKind: testCase.source || 'fixture',
    generatedBy: testCase.source || null,
    referencePng,
    referencePngPath: pngPath,
    settings: {
      width: testCase.width,
      ...(testCase.height ? { height: testCase.height } : {}),
      mozjpegQuality: testCase.mozjpegQuality,
      jpegliDistance: testCase.jpegliDistance,
    },
    expectations: buildBakeoffCorpusExpectations(testCase),
  });

  const mozjpegTimes = [];
  const jpegliTimes = [];
  let mozjpegData = null;
  let jpegliData = null;

  for (let i = 0; i < options.iterations; i += 1) {
    const mozjpeg = await encodeMozjpeg(referencePng, testCase.mozjpegQuality);
    const jpegliOutputPath = path.join(TEMP_DIR, `${testCase.label}-${i}.jpegli.jpg`);
    const jpegli = runCjpegli(options.cjpegli, pngPath, jpegliOutputPath, testCase.jpegliDistance);

    mozjpegTimes.push(mozjpeg.encodeMs);
    jpegliTimes.push(jpegli.encodeMs);
    mozjpegData = mozjpeg.data;
    jpegliData = jpegli.data;
  }

  const [mozjpegQuality, jpegliQuality] = await Promise.all([
    calculateQualityMetrics(mozjpegData, referencePng),
    calculateQualityMetrics(jpegliData, referencePng),
  ]);
  const tuning = await runDistanceTuning({
    testCase,
    options,
    referencePng,
    pngPath,
    mozjpegData,
    mozjpegQuality,
  });

  return {
    corpusEntry,
    result: {
      label: testCase.label,
      input: inputLabel(testCase),
      width: testCase.width,
      settings: {
        mozjpegQuality: testCase.mozjpegQuality,
        jpegliDistance: testCase.jpegliDistance,
      },
      commands: {
        mozjpeg: `ImageEngine.from(referencePng).toBuffer('jpeg', ${testCase.mozjpegQuality})`,
        jpegli: [
          options.cjpegli,
          path.relative(resolveRoot(), pngPath),
          path.relative(resolveRoot(), path.join(TEMP_DIR, `${testCase.label}-<iteration>.jpegli.jpg`)),
          '--distance',
          String(testCase.jpegliDistance),
          '--quiet',
        ].join(' '),
      },
      mozjpeg: {
        bytes: mozjpegData.length,
        avgEncodeMs: average(mozjpegTimes),
        ssim: mozjpegQuality.ssim,
        psnr: mozjpegQuality.psnr,
      },
      jpegli: {
        bytes: jpegliData.length,
        avgEncodeMs: average(jpegliTimes),
        ssim: jpegliQuality.ssim,
        psnr: jpegliQuality.psnr,
      },
      delta: {
        bytesPercent: ((jpegliData.length - mozjpegData.length) / Math.max(mozjpegData.length, 1)) * 100,
        encodeMsPercent:
          ((average(jpegliTimes) - average(mozjpegTimes)) / Math.max(average(mozjpegTimes), 0.001)) * 100,
        ssim: jpegliQuality.ssim - mozjpegQuality.ssim,
        psnr: jpegliQuality.psnr - mozjpegQuality.psnr,
      },
      tuning,
    },
  };
}

function buildMarkdown(report) {
  const lines = [
    '# JPEG Backend Bake-Off',
    '',
    `Generated: ${report.generatedAt}`,
    `Node: ${report.environment.node}`,
    `Platform: ${report.environment.platform}/${report.environment.arch}`,
    `lazy-image: ${report.versions.lazyImage}`,
    `mozjpeg crate: ${report.versions.mozjpeg || 'unknown'}`,
    `mozjpeg-sys crate: ${report.versions.mozjpegSys || 'unknown'}`,
    `jpegli: ${report.versions.jpegli || 'unknown'}`,
    `cjpegli: ${report.tools.cjpegli}`,
    `cjpegli probe: ${report.tools.cjpegliProbe || 'unknown'}`,
    `Iterations: ${report.iterations}`,
    '',
    ...buildPackageImpactMarkdown(report.packageImpact),
    '',
    ...buildCorpusManifestMarkdown(report.corpus),
    '',
    'Command shape: `cjpegli INPUT OUTPUT --distance <distance> --quiet`',
    `Distance tuning: ${report.tuning.enabled ? 'enabled' : 'disabled'}`,
  ];

  if (report.tuning.enabled) {
    lines.push(
      `Requested distance candidates: ${report.tuning.requestedDistanceCandidates.join(', ')}`,
      'Matched-byte target: minimize absolute byte delta against the MozJPEG output for the same resized reference PNG.',
      'Matched-quality target: minimize absolute SSIM delta against the MozJPEG output for the same resized reference PNG.',
      ''
    );
  } else if (report.tuning.reason) {
    lines.push(`Distance tuning reason: ${report.tuning.reason}`, '');
  } else {
    lines.push('');
  }

  lines.push(
    '| Case | Input | Width | MozJPEG q | jpegli distance | MozJPEG bytes | jpegli bytes | jpegli bytes delta | MozJPEG ms | jpegli ms | jpegli ms delta | MozJPEG SSIM | jpegli SSIM | SSIM delta | MozJPEG PSNR | jpegli PSNR | PSNR delta |',
    '|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|'
  );

  for (const result of report.results) {
    lines.push(
      [
        result.label,
        result.input,
        result.width,
        result.settings.mozjpegQuality,
        result.settings.jpegliDistance,
        result.mozjpeg.bytes,
        result.jpegli.bytes,
        formatPercent(result.delta.bytesPercent),
        formatNumber(result.mozjpeg.avgEncodeMs, 1),
        formatNumber(result.jpegli.avgEncodeMs, 1),
        formatPercent(result.delta.encodeMsPercent),
        formatNumber(result.mozjpeg.ssim, 5),
        formatNumber(result.jpegli.ssim, 5),
        formatNumber(result.delta.ssim, 5),
        formatNumber(result.mozjpeg.psnr, 2),
        formatNumber(result.jpegli.psnr, 2),
        formatNumber(result.delta.psnr, 2),
      ].join(' | ').replace(/^/, '| ').replace(/$/, ' |')
    );
  }

  if (report.tuning.enabled) {
    lines.push(
      '',
      '## Distance Tuning Summary',
      '',
      '| Case | Candidates | Matched-byte distance | Matched-byte bytes delta | Matched-byte SSIM delta | Matched-quality distance | Matched-quality bytes delta | Matched-quality SSIM delta |',
      '|---|---:|---:|---:|---:|---:|---:|---:|'
    );

    for (const result of report.results) {
      lines.push(
        [
          result.label,
          result.tuning.distanceCandidates.length,
          result.tuning.matchedBytes.distance,
          formatPercent(result.tuning.matchedBytes.delta.bytesPercent),
          formatNumber(result.tuning.matchedBytes.delta.ssim, 5),
          result.tuning.matchedQuality.distance,
          formatPercent(result.tuning.matchedQuality.delta.bytesPercent),
          formatNumber(result.tuning.matchedQuality.delta.ssim, 5),
        ].join(' | ').replace(/^/, '| ').replace(/$/, ' |')
      );
    }
  }

  lines.push(
    '',
    'Decision rule: do not promote jpegli unless it improves bytes and objective quality without unacceptable encode-time, RSS, package-size, or build regressions.'
  );

  return `${lines.join('\n')}\n`;
}

function writeReport(report, outputDir) {
  fs.mkdirSync(outputDir, { recursive: true });

  const jsonPath = path.join(outputDir, `${OUTPUT_FILE_BASE}.json`);
  const mdPath = path.join(outputDir, `${OUTPUT_FILE_BASE}.md`);

  fs.writeFileSync(jsonPath, JSON.stringify(report, null, 2));
  fs.writeFileSync(mdPath, buildMarkdown(report));

  return { jsonPath, mdPath };
}

async function main() {
  const options = {
    cjpegli: getArg('--cjpegli', process.env.JPEGLI_CJPEGLI || 'cjpegli'),
    jpegliVersion: getArg('--jpegli-version', process.env.JPEGLI_VERSION || null),
    iterations: parsePositiveInteger(
      getArg('--iterations', process.env.JPEGLI_BAKEOFF_ITERATIONS || String(DEFAULT_ITERATIONS)),
      '--iterations'
    ),
    outputDir: path.resolve(getArg('--output-dir', resolveRoot('artifacts', 'benchmark'))),
    allowMissing: hasFlag('--allow-missing-jpegli') || process.env.JPEGLI_ALLOW_MISSING === '1',
    tuneDistances: !hasFlag('--skip-distance-tuning') && process.env.JPEGLI_SKIP_DISTANCE_TUNING !== '1',
    distanceCandidates: parseDistanceCandidateList(
      getArg(
        '--distance-candidates',
        process.env.JPEGLI_DISTANCE_CANDIDATES || DEFAULT_JPEGLI_DISTANCE_CANDIDATES.join(',')
      ),
      '--distance-candidates'
    ),
  };

  const probe = probeCjpegli(options.cjpegli, options.allowMissing);
  if (!probe.available) {
    console.log(`Skipping JPEG backend bake-off: ${probe.reason}`);
    console.log('Set JPEGLI_CJPEGLI=/path/to/cjpegli or install cjpegli to generate benchmark artifacts.');
    return;
  }

  fs.mkdirSync(TEMP_DIR, { recursive: true });

  console.log('=== JPEG Backend Bake-Off ===');
  console.log(`cjpegli: ${options.cjpegli}`);
  console.log(`Iterations: ${options.iterations}`);
  console.log(`Distance tuning: ${options.tuneDistances ? 'enabled' : 'disabled'}`);
  console.log(`Output directory: ${options.outputDir}\n`);
  if (!options.jpegliVersion && !probe.version) {
    console.log('Warning: cjpegli version is unknown. Set JPEGLI_VERSION=<tag-or-commit> for publishable evidence.');
  }

  const results = [];
  const corpus = [];
  for (const testCase of CASES) {
    console.log(`- ${testCase.label}`);
    const { result, corpusEntry } = await runCase(testCase, options);
    results.push(result);
    corpus.push(corpusEntry);
  }

  const report = {
    generatedAt: new Date().toISOString(),
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
    },
    versions: {
      lazyImage: readPackageVersion(),
      mozjpeg: readCargoLockVersion('mozjpeg'),
      mozjpegSys: readCargoLockVersion('mozjpeg-sys'),
      jpegli: options.jpegliVersion || probe.version || 'unknown',
    },
    tools: {
      cjpegli: options.cjpegli,
      cjpegliProbe: probe.help,
    },
    iterations: options.iterations,
    packageImpact: collectBenchmarkPackageImpact({
      benchmarkTool: 'cjpegli',
      candidateBackend: 'jpegli',
    }),
    tuning: {
      enabled: options.tuneDistances,
      reason: options.tuneDistances
        ? null
        : 'disabled by --skip-distance-tuning or JPEGLI_SKIP_DISTANCE_TUNING=1',
      requestedDistanceCandidates: options.distanceCandidates,
      runsPerCandidate: 1,
      matchedBytesMetric: 'absolute output byte delta vs MozJPEG',
      matchedQualityMetric: 'absolute SSIM delta vs MozJPEG',
    },
    corpus,
    commandShape: 'cjpegli INPUT OUTPUT --distance <distance> --quiet',
    timingNote:
      'MozJPEG time measures in-process encode from the resized reference PNG buffer; jpegli time measures cjpegli process wall time from the same resized reference PNG file.',
    results,
  };

  const { jsonPath, mdPath } = writeReport(report, options.outputDir);

  console.log('\n| Case | MozJPEG bytes | jpegli bytes | bytes delta | MozJPEG ms | jpegli ms | ms delta | jpegli SSIM delta | Matched-byte delta | Matched-quality byte delta |');
  console.log('|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|');
  for (const result of results) {
    console.log(
      [
        result.label,
        result.mozjpeg.bytes,
        result.jpegli.bytes,
        formatPercent(result.delta.bytesPercent),
        formatNumber(result.mozjpeg.avgEncodeMs, 1),
        formatNumber(result.jpegli.avgEncodeMs, 1),
        formatPercent(result.delta.encodeMsPercent),
        formatNumber(result.delta.ssim, 5),
        result.tuning.enabled ? formatPercent(result.tuning.matchedBytes.delta.bytesPercent) : 'n/a',
        result.tuning.enabled ? formatPercent(result.tuning.matchedQuality.delta.bytesPercent) : 'n/a',
      ].join(' | ').replace(/^/, '| ').replace(/$/, ' |')
    );
  }

  console.log(`\nJSON saved: ${jsonPath}`);
  console.log(`Markdown saved: ${mdPath}`);
  console.log('JPEG backend bake-off completed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
