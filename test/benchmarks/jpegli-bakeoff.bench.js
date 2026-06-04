const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const sharp = require('sharp');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');
const { calculateQualityMetrics } = require('../helpers/quality');
const { buildPackageImpactMarkdown, collectBenchmarkPackageImpact } = require('../helpers/package-impact');
const { ImageEngine } = require(resolveRoot('index'));

const DEFAULT_ITERATIONS = 3;
const OUTPUT_FILE_BASE = 'jpegli-bakeoff';
const TEMP_DIR = resolveTemp('benchmarks', OUTPUT_FILE_BASE);

const CASES = [
  {
    label: 'large-png-default-jpeg',
    input: resolveFixture('test_4.5MB_5000x5000.png'),
    width: 800,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
  },
  {
    label: 'small-jpeg-default-jpeg',
    input: resolveFixture('test_38kb_input.jpg'),
    width: 800,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
  },
  {
    label: 'mid-png-smaller-output',
    input: resolveFixture('test_100KB_1188x1188.png'),
    width: 400,
    mozjpegQuality: 75,
    jpegliDistance: 1.4,
  },
];

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
  const version = versionResult.error ? null : firstLine(versionResult.stdout || versionResult.stderr || '');

  return {
    available: true,
    help: firstLine(result.stdout || result.stderr || ''),
    version,
  };
}

async function buildReferencePng(testCase) {
  return sharp(testCase.input)
    .resize(testCase.width, null, { withoutEnlargement: true })
    .png({ compressionLevel: 0 })
    .toBuffer();
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

async function runCase(testCase, options) {
  const referencePng = await buildReferencePng(testCase);
  const pngPath = path.join(TEMP_DIR, `${testCase.label}.png`);
  fs.writeFileSync(pngPath, referencePng);

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

  return {
    label: testCase.label,
    input: path.relative(resolveRoot(), testCase.input),
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
    'Command shape: `cjpegli INPUT OUTPUT --distance <distance> --quiet`',
    '',
    '| Case | Input | Width | MozJPEG q | jpegli distance | MozJPEG bytes | jpegli bytes | jpegli bytes delta | MozJPEG ms | jpegli ms | jpegli ms delta | MozJPEG SSIM | jpegli SSIM | SSIM delta | MozJPEG PSNR | jpegli PSNR | PSNR delta |',
    '|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|',
  ];

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
  console.log(`Output directory: ${options.outputDir}\n`);
  if (!options.jpegliVersion && !probe.version) {
    console.log('Warning: cjpegli version is unknown. Set JPEGLI_VERSION=<tag-or-commit> for publishable evidence.');
  }

  const results = [];
  for (const testCase of CASES) {
    console.log(`- ${testCase.label}`);
    results.push(await runCase(testCase, options));
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
    commandShape: 'cjpegli INPUT OUTPUT --distance <distance> --quiet',
    timingNote:
      'MozJPEG time measures in-process encode from the resized reference PNG buffer; jpegli time measures cjpegli process wall time from the same resized reference PNG file.',
    results,
  };

  const { jsonPath, mdPath } = writeReport(report, options.outputDir);

  console.log('\n| Case | MozJPEG bytes | jpegli bytes | bytes delta | MozJPEG ms | jpegli ms | ms delta | jpegli SSIM delta |');
  console.log('|---|---:|---:|---:|---:|---:|---:|---:|');
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
