const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { performance } = require('perf_hooks');

const sharp = require('sharp');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { calculateQualityMetrics } = require('../helpers/quality');

const OUTPUT_DIR = resolveRoot('artifacts', 'benchmark');
const JSON_OUTPUT = path.join(OUTPUT_DIR, 'wasm-upload-summary.json');
const MARKDOWN_OUTPUT = path.join(OUTPUT_DIR, 'wasm-upload-summary.md');

const RUNTIME_FILTERS = new Map([
  ['all', ['browser-worker', 'edge-isolate']],
  ['browser', ['browser-worker']],
  ['edge', ['edge-isolate']],
]);

const SCENARIOS = [
  {
    id: 'large-photo-upload-webp',
    label: 'Large JPEG upload -> WebP',
    inputPath: resolveFixture('test_3.2MB_5000x5000.jpg'),
    inputKind: 'large-photo-jpeg',
    outputFormat: 'webp',
    maxWidth: 1600,
    maxHeight: 1600,
    targetBytes: 500_000,
    minQuality: 45,
    maxQuality: 86,
  },
  {
    id: 'large-png-upload-jpeg',
    label: 'Large PNG upload -> JPEG',
    inputPath: resolveFixture('test_4.5MB_5000x5000.png'),
    inputKind: 'large-rgba-png',
    outputFormat: 'jpeg',
    maxWidth: 1600,
    maxHeight: 1600,
    targetBytes: 450_000,
    minQuality: 45,
    maxQuality: 90,
  },
];

const WASM_CODEC_BASELINES = [
  {
    packageName: 'jSquash',
    packages: ['@jsquash/jpeg', '@jsquash/webp', '@jsquash/resize'],
    reason: 'Optional jSquash packages are not installed in this workspace.',
  },
  {
    packageName: 'Squoosh',
    packages: ['@squoosh/lib'],
    reason: 'Optional Squoosh package is not installed in this workspace.',
  },
];

const BROWSER_COMPRESSOR_BASELINES = [
  {
    packageName: 'browser-image-compression',
    packages: ['browser-image-compression'],
    reason: 'browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here.',
  },
  {
    packageName: 'Compressor.js',
    packages: ['compressorjs'],
    reason: 'Compressor.js requires DOM Canvas APIs and is not installed here.',
  },
];

let nativeModuleLoad = null;

function getArg(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index === process.argv.length - 1) {
    return fallback;
  }
  return process.argv[index + 1];
}

function ensureOutputDir() {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

function loadNativeModule() {
  if (nativeModuleLoad) {
    return { ...nativeModuleLoad, cached: true, loadMs: 0 };
  }

  const start = performance.now();
  const mod = require(resolveRoot('index'));
  nativeModuleLoad = {
    mod,
    cached: false,
    loadMs: performance.now() - start,
  };
  return nativeModuleLoad;
}

function formatBytes(bytes) {
  if (!Number.isFinite(bytes)) {
    return 'n/a';
  }
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${bytes} B`;
}

function formatMs(ms) {
  return Number.isFinite(ms) ? ms.toFixed(1) : 'n/a';
}

function formatMaybe(value, digits = 3) {
  if (value === null || value === undefined) {
    return 'n/a';
  }
  if (typeof value === 'boolean') {
    return value ? 'yes' : 'no';
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value.toFixed(digits) : String(value);
  }
  return String(value);
}

function markdownEscape(value) {
  return String(value ?? 'n/a').replace(/\|/g, '\\|');
}

function resolvePackageRoot(packageName) {
  const searchPaths = [resolveRoot()];
  const candidates = [`${packageName}/package.json`, packageName];

  for (const candidate of candidates) {
    try {
      const resolved = require.resolve(candidate, { paths: searchPaths });
      let current = fs.statSync(resolved).isDirectory() ? resolved : path.dirname(resolved);
      while (current !== path.dirname(current)) {
        const packageJsonPath = path.join(current, 'package.json');
        if (fs.existsSync(packageJsonPath)) {
          try {
            const pkg = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));
            if (pkg.name === packageName) {
              return current;
            }
          } catch {
            return current;
          }
        }
        current = path.dirname(current);
      }
    } catch {
      // Try the next candidate.
    }
  }

  return null;
}

function collectFiles(root, dir, files) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectFiles(root, fullPath, files);
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }
}

function directorySize(root) {
  const files = [];
  collectFiles(root, root, files);
  return files.reduce((total, file) => total + fs.statSync(file).size, 0);
}

function gzipSizeOfDirectory(root) {
  const files = [];
  collectFiles(root, root, files);
  const chunks = [];

  for (const file of files.sort()) {
    chunks.push(Buffer.from(`\n--- ${path.relative(root, file)} ---\n`));
    chunks.push(fs.readFileSync(file));
  }

  return zlib.gzipSync(Buffer.concat(chunks), { level: 9 }).length;
}

function getPackageInfo(packageName) {
  const root = resolvePackageRoot(packageName);
  if (!root) {
    return {
      packageName,
      available: false,
      root: null,
      bundleBytes: null,
      bundleGzipBytes: null,
    };
  }

  return {
    packageName,
    available: true,
    root,
    bundleBytes: directorySize(root),
    bundleGzipBytes: gzipSizeOfDirectory(root),
  };
}

function sumNullable(values) {
  const finite = values.filter((value) => Number.isFinite(value));
  return finite.length === values.length ? finite.reduce((sum, value) => sum + value, 0) : null;
}

async function buildReferenceBuffer(scenario) {
  return sharp(scenario.inputPath)
    .rotate()
    .resize({
      width: scenario.maxWidth,
      height: scenario.maxHeight,
      fit: 'inside',
      withoutEnlargement: true,
    })
    .png({ compressionLevel: 0 })
    .toBuffer();
}

async function inspectMetadata(buffer) {
  const metadata = await sharp(buffer).metadata();
  const hasMetadata = Boolean(metadata.exif || metadata.icc || metadata.iptc || metadata.xmp);
  return {
    hasMetadata,
    stripped: !hasMetadata,
    exif: Boolean(metadata.exif),
    icc: Boolean(metadata.icc),
    iptc: Boolean(metadata.iptc),
    xmp: Boolean(metadata.xmp),
  };
}

async function runNativeReference(scenario) {
  if (!fs.existsSync(scenario.inputPath)) {
    throw new Error(`Fixture not found: ${scenario.inputPath}`);
  }

  const rssBefore = process.memoryUsage().rss;
  const totalStart = performance.now();
  const native = loadNativeModule();
  const { ImageEngine } = native.mod;
  const instantiateMs = native.cached ? 0 : native.loadMs;

  const encodeStart = performance.now();
  const result = await ImageEngine.fromPath(scenario.inputPath)
    .resize({
      width: scenario.maxWidth,
      height: scenario.maxHeight,
      fit: 'inside',
    })
    .toBufferTargetBytes(scenario.outputFormat, {
      targetBytes: scenario.targetBytes,
      minQuality: scenario.minQuality,
      maxQuality: scenario.maxQuality,
      qualityFloorPolicy: 'best-effort',
    });
  const firstEncodeMs = performance.now() - encodeStart;
  const totalMs = performance.now() - totalStart;
  const rssAfter = process.memoryUsage().rss;

  const [referenceBuffer, metadata] = await Promise.all([
    buildReferenceBuffer(scenario),
    inspectMetadata(result.data),
  ]);
  const quality = await calculateQualityMetrics(result.data, referenceBuffer);

  return {
    scenario: scenario.id,
    scenarioLabel: scenario.label,
    runtime: 'node-native',
    package: '@alberteinshutoin/lazy-image',
    baselineType: 'native-reference',
    status: 'ok',
    inputBytes: fs.statSync(scenario.inputPath).size,
    inputKind: scenario.inputKind,
    outputFormat: scenario.outputFormat,
    targetBytes: scenario.targetBytes,
    maxWidth: scenario.maxWidth,
    maxHeight: scenario.maxHeight,
    bundleBytes: null,
    bundleGzipBytes: null,
    instantiateMs,
    firstEncodeMs,
    encodeMs: result.metrics?.encodeMs ?? null,
    totalMs,
    bytesOut: result.bytesOut,
    targetHit: result.budgetMet,
    quality: result.quality,
    ssim: quality.ssim,
    psnr: quality.psnr,
    metadataStripped: metadata.stripped,
    memory: {
      rssBeforeBytes: rssBefore,
      rssAfterBytes: rssAfter,
      rssDeltaBytes: Math.max(0, rssAfter - rssBefore),
      note: 'Node RSS delta, not browser peak memory.',
    },
    notes: 'Native Node reference for future Wasm/browser comparisons.',
  };
}

function makeOptionalBaselineRow({ scenario, runtime, baselineType, packageName, packages, reason }) {
  const packageInfos = packages.map(getPackageInfo);
  const available = packageInfos.every((info) => info.available);
  const installedPackages = packageInfos.filter((info) => info.available).map((info) => info.packageName);
  const missingPackages = packageInfos.filter((info) => !info.available).map((info) => info.packageName);

  return {
    scenario: scenario.id,
    scenarioLabel: scenario.label,
    runtime,
    package: packageName,
    baselineType,
    status: available ? 'not-run' : 'unavailable',
    inputBytes: fs.existsSync(scenario.inputPath) ? fs.statSync(scenario.inputPath).size : null,
    inputKind: scenario.inputKind,
    outputFormat: scenario.outputFormat,
    targetBytes: scenario.targetBytes,
    maxWidth: scenario.maxWidth,
    maxHeight: scenario.maxHeight,
    bundleBytes: available ? sumNullable(packageInfos.map((info) => info.bundleBytes)) : null,
    bundleGzipBytes: available ? sumNullable(packageInfos.map((info) => info.bundleGzipBytes)) : null,
    instantiateMs: null,
    firstEncodeMs: null,
    encodeMs: null,
    totalMs: null,
    bytesOut: null,
    targetHit: null,
    quality: null,
    ssim: null,
    psnr: null,
    metadataStripped: null,
    memory: {
      rssBeforeBytes: null,
      rssAfterBytes: null,
      rssDeltaBytes: null,
      note: 'Unavailable in this Node-local harness.',
    },
    packageAvailability: {
      installed: installedPackages,
      missing: missingPackages,
    },
    notes: available
      ? `${packageName} is installed, but this first harness records package size only; add a runtime adapter before making performance claims.`
      : reason,
  };
}

async function run(runtimeFilter) {
  const runtimes = RUNTIME_FILTERS.get(runtimeFilter);
  if (!runtimes) {
    throw new Error(`Unsupported --runtime value: ${runtimeFilter}. Use one of: ${[...RUNTIME_FILTERS.keys()].join(', ')}`);
  }

  ensureOutputDir();

  const rows = [];
  for (const scenario of SCENARIOS) {
    console.log(`Scenario: ${scenario.label}`);
    rows.push(await runNativeReference(scenario));

    for (const runtime of runtimes) {
      for (const baseline of WASM_CODEC_BASELINES) {
        rows.push(
          makeOptionalBaselineRow({
            scenario,
            runtime,
            baselineType: 'wasm-codec',
            ...baseline,
          })
        );
      }

      for (const baseline of BROWSER_COMPRESSOR_BASELINES) {
        rows.push(
          makeOptionalBaselineRow({
            scenario,
            runtime,
            baselineType: 'browser-compressor',
            ...baseline,
          })
        );
      }
    }
  }

  writeReports({
    runtimeFilter,
    rows,
  });

  printSummary(rows);
}

function writeReports({ runtimeFilter, rows }) {
  const generatedAt = new Date().toISOString();
  const jsonReport = {
    generatedAt,
    command: 'node test/benchmarks/wasm-upload-comparison.bench.js',
    runtimeFilter,
    artifactPaths: {
      json: path.relative(resolveRoot(), JSON_OUTPUT),
      markdown: path.relative(resolveRoot(), MARKDOWN_OUTPUT),
    },
    metrics: [
      'bundleBytes',
      'bundleGzipBytes',
      'instantiateMs',
      'firstEncodeMs',
      'encodeMs',
      'totalMs',
      'bytesOut',
      'targetHit',
      'quality',
      'ssim',
      'psnr',
      'metadataStripped',
      'memory.rssDeltaBytes',
    ],
    scenarios: SCENARIOS.map((scenario) => ({
      id: scenario.id,
      label: scenario.label,
      input: path.relative(resolveRoot(), scenario.inputPath),
      outputFormat: scenario.outputFormat,
      maxWidth: scenario.maxWidth,
      maxHeight: scenario.maxHeight,
      targetBytes: scenario.targetBytes,
      qualityRange: [scenario.minQuality, scenario.maxQuality],
    })),
    rows,
  };

  fs.writeFileSync(JSON_OUTPUT, `${JSON.stringify(jsonReport, null, 2)}\n`);
  fs.writeFileSync(MARKDOWN_OUTPUT, renderMarkdownReport(jsonReport));
}

function renderMarkdownReport(report) {
  const lines = [
    '# Wasm Upload Benchmark Summary',
    '',
    `Generated: ${report.generatedAt}`,
    '',
    'This report is generated by `npm run test:bench:wasm` and is intended for browser/Edge upload-preflight comparisons.',
    'Rows marked `unavailable` document optional competitor baselines that are not installed or cannot execute in the current Node-local harness.',
    '',
    '| Scenario | Runtime | Package | Type | Status | Bundle gzip | Instantiate | First encode | Total | Bytes out | Target hit | Quality | SSIM | PSNR | Metadata stripped | Memory delta | Notes |',
    '|---|---|---|---|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---:|---|',
  ];

  for (const row of report.rows) {
    const cells = [
      row.scenario,
      row.runtime,
      row.package,
      row.baselineType,
      row.status,
      formatBytes(row.bundleGzipBytes),
      formatMs(row.instantiateMs),
      formatMs(row.firstEncodeMs),
      formatMs(row.totalMs),
      formatBytes(row.bytesOut),
      formatMaybe(row.targetHit, 0),
      formatMaybe(row.quality, 0),
      formatMaybe(row.ssim, 4),
      formatMaybe(row.psnr, 2),
      formatMaybe(row.metadataStripped, 0),
      formatBytes(row.memory?.rssDeltaBytes),
      row.notes,
    ].map(markdownEscape);
    lines.push(`| ${cells.join(' | ')} |`);
  }

  lines.push('');
  return `${lines.join('\n')}\n`;
}

function printSummary(rows) {
  console.log('');
  console.log(`Wrote ${path.relative(resolveRoot(), JSON_OUTPUT)}`);
  console.log(`Wrote ${path.relative(resolveRoot(), MARKDOWN_OUTPUT)}`);
  console.log('');

  for (const row of rows.filter((item) => item.status === 'ok')) {
    console.log(
      `${row.scenario} ${row.package}: ${formatBytes(row.bytesOut)} in ${formatMs(row.totalMs)}ms, targetHit=${formatMaybe(row.targetHit)}`
    );
  }

  const unavailable = rows.filter((row) => row.status === 'unavailable').length;
  const notRun = rows.filter((row) => row.status === 'not-run').length;
  console.log(`Optional baselines: ${unavailable} unavailable, ${notRun} installed but not run`);
}

run(getArg('--runtime', 'all')).catch((err) => {
  console.error(err.stack || err.message);
  process.exit(1);
});
