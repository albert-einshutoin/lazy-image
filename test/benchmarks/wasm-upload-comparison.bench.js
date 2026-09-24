const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { createHash } = require('crypto');
const { execFileSync } = require('child_process');
const { performance } = require('perf_hooks');

const sharp = require('sharp');
const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { calculateQualityMetrics } = require('../helpers/quality');

const OUTPUT_DIR = resolveRoot('artifacts', 'benchmark');
const JSON_OUTPUT = path.join(OUTPUT_DIR, 'wasm-upload-summary.json');
const MARKDOWN_OUTPUT = path.join(OUTPUT_DIR, 'wasm-upload-summary.md');

const RUNTIME_FILTERS = new Map([
  ['all', ['node-wasm', 'browser-worker']],
  ['node', ['node-wasm']],
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
      packageDirectoryBytes: null,
      packageDirectoryGzipBytes: null,
    };
  }

  return {
    packageName,
    available: true,
    root,
    packageDirectoryBytes: directorySize(root),
    packageDirectoryGzipBytes: gzipSizeOfDirectory(root),
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

  const [referenceBuffer, inputMetadata, outputMetadata] = await Promise.all([
    buildReferenceBuffer(scenario),
    inspectMetadata(fs.readFileSync(scenario.inputPath)),
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
    browserBundleBytes: null,
    browserBundleGzipBytes: null,
    packageDirectoryBytes: null,
    packageDirectoryGzipBytes: null,
    instantiateMs,
    firstEncodeMs,
    encodeMs: result.metrics?.encodeMs ?? null,
    totalMs,
    bytesOut: result.bytesOut,
    targetHit: result.budgetMet,
    quality: result.quality,
    ssim: quality.ssim,
    psnr: quality.psnr,
    metadataStripped: inputMetadata.hasMetadata ? outputMetadata.stripped : null,
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
    browserBundleBytes: null,
    browserBundleGzipBytes: null,
    packageDirectoryBytes: available ? sumNullable(packageInfos.map((info) => info.packageDirectoryBytes)) : null,
    packageDirectoryGzipBytes: available ? sumNullable(packageInfos.map((info) => info.packageDirectoryGzipBytes)) : null,
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
      ? `${packageName} is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims.`
      : reason,
  };
}

function publishedRow(scenario, runtime, evidence) {
  const evidenceId = scenario.id === 'large-photo-upload-webp' ? 'jpeg-webp' : 'png-jpeg';
  const result = runtime === 'node-wasm'
    ? evidence.nodeResults?.find((entry) => entry.id === evidenceId)
    : evidence.browserResults?.results.find((entry) => entry.id === evidenceId);
  if (!result) throw new Error(`Missing published evidence: ${runtime}/${evidenceId}`);
  const fixture = evidence.fixtures.find((item) => item.id === evidenceId);
  if (!fixture) throw new Error(`Missing published fixture: ${evidenceId}`);
  return {
    scenario: scenario.id,
    scenarioLabel: scenario.label,
    runtime,
    package: '@alberteinshutoin/lazy-image-wasm@' + evidence.publishedVersion,
    baselineType: 'published-package',
    status: 'ok',
    inputBytes: fixture.inputBytes,
    inputKind: scenario.inputKind,
    outputFormat: scenario.outputFormat,
    targetBytes: scenario.targetBytes,
    maxWidth: scenario.maxWidth,
    maxHeight: scenario.maxHeight,
    browserBundleBytes: runtime === 'browser-worker' ? evidence.browserResults.deploymentRawBytes : null,
    browserBundleGzipBytes: runtime === 'browser-worker' ? evidence.browserResults.deploymentGzipBytes : null,
    packageDirectoryBytes: runtime === 'browser-worker' ? evidence.browserResults.packageDirectoryBytes : null,
    packageDirectoryGzipBytes: null,
    instantiateMs: result.metrics.instantiateMs,
    firstEncodeMs: result.metrics.firstEncodeMs,
    encodeMs: result.metrics.encodeMs,
    totalMs: result.metrics.totalMs,
    firstVisibleMs: result.coldFromBeforeWorkerMs ?? null,
    warmMedianMs: result.warmMedianMs,
    bytesOut: result.output.bytes,
    targetHit: result.metrics.budgetMet,
    quality: result.metrics.qualityUsed,
    ssim: null,
    psnr: null,
    metadataStripped: null,
    memory: { rssDeltaBytes: null, note: 'Wasm/browser peak memory unavailable.' },
    notes: `Published npm; inspected output ${result.output.sha256}; full conditions in wasm-published-evidence.json`,
  };
}

function metadataVerification(evidence, runtimes) {
  const fixture = evidence.fixtures.find((item) => item.id === 'metadata-budget');
  if (!fixture?.inputMetadata || ['exif', 'gpsTag', 'xmp', 'icc'].some((key) => fixture.inputMetadata[key] !== true)) {
    throw new Error('Metadata evidence input must contain EXIF, GPS, XMP, and ICC');
  }
  const results = {};
  for (const runtime of runtimes) {
    const result = runtime === 'node-wasm'
      ? evidence.nodeResults?.find((entry) => entry.id === 'metadata-budget')
      : evidence.browserResults?.results.find((entry) => entry.id === 'metadata-budget');
    if (!result) throw new Error(`Missing metadata evidence: ${runtime}`);
    const output = result.output.metadata;
    if (output.exif !== false || output.xmp !== false || output.icc !== false) {
      throw new Error(`Metadata output retained a required field: ${runtime}`);
    }
    results[runtime] = { outputMetadata: output, outputSha256: result.output.sha256,
      removed: { exif: true, gpsTag: true, xmp: true, icc: true } };
  }
  return { scenario: 'metadata-budget', inputSha256: fixture.inputSha256,
    inputMetadata: fixture.inputMetadata, results,
    rawEvidence: 'docs/history/wasm-1.3.1/wasm-published-evidence.json' };
}

async function run(runtimeFilter) {
  const runtimes = RUNTIME_FILTERS.get(runtimeFilter);
  if (!runtimes) {
    throw new Error(`Unsupported --runtime value: ${runtimeFilter}. Use one of: ${[...RUNTIME_FILTERS.keys()].join(', ')}`);
  }

  ensureOutputDir();

  // Required runtime rows must come from the published package, not from the checkout.
  const { collectPublishedWasmEvidence } = await import('./wasm-published-evidence.mjs');
  const evidence = await collectPublishedWasmEvidence({
    version: getArg('--version', process.env.WASM_BENCH_VERSION || require('../../package.json').version),
    runtime: runtimeFilter,
    chromePath: getArg('--chrome', process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'),
  });

  const rows = [];
  for (const scenario of SCENARIOS) {
    console.log(`Scenario: ${scenario.label}`);
    try {
      rows.push(await runNativeReference(scenario));
    } catch (error) {
      rows.push({ scenario: scenario.id, scenarioLabel: scenario.label, runtime: 'node-native',
        package: '@alberteinshutoin/lazy-image', baselineType: 'native-reference',
        status: 'unavailable', notes: `Optional local native reference: ${error.message}` });
    }

    for (const runtime of runtimes.filter((item) => item !== 'edge-isolate')) {
      rows.push(publishedRow(scenario, runtime, evidence));
    }

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
    evidence,
  });

  printSummary(rows);
}

function writeReports({ runtimeFilter, rows, evidence }) {
  const generatedAt = new Date().toISOString();
  const jsonReport = {
    generatedAt,
    command: evidence.command,
    sourceSha: evidence.sourceSha,
    publishedVersion: evidence.publishedVersion,
    runtimeFilter,
    artifactPaths: {
      json: path.relative(resolveRoot(), JSON_OUTPUT),
      markdown: path.relative(resolveRoot(), MARKDOWN_OUTPUT),
      publishedEvidence: 'artifacts/benchmark/wasm-published-evidence.json',
    },
    metrics: [
      'browserBundleBytes',
      'browserBundleGzipBytes',
      'packageDirectoryBytes',
      'packageDirectoryGzipBytes',
      'instantiateMs',
      'firstEncodeMs',
      'encodeMs',
      'totalMs',
      'firstVisibleMs',
      'warmMedianMs',
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
    metadataVerification: metadataVerification(evidence, RUNTIME_FILTERS.get(runtimeFilter)),
    rows,
  };
  writeReportFiles(jsonReport);
}

function writeReportFiles(jsonReport) {
  fs.writeFileSync(JSON_OUTPUT, `${JSON.stringify(jsonReport, null, 2)}\n`);
  fs.writeFileSync(MARKDOWN_OUTPUT, renderMarkdownReport(jsonReport));
}

function reaggregate(evidencePath, previousSummaryPath) {
  if (!evidencePath || !previousSummaryPath) {
    throw new Error('--reaggregate and --previous-summary are both required');
  }
  ensureOutputDir();
  const evidenceBytes = fs.readFileSync(evidencePath);
  const previousBytes = fs.readFileSync(previousSummaryPath);
  const evidence = JSON.parse(evidenceBytes);
  const previous = JSON.parse(previousBytes);
  if (evidence.verdict !== 'PASS' || evidence.sourceSha !== previous.sourceSha ||
      evidence.publishedVersion !== previous.publishedVersion || previous.runtimeFilter !== 'all') {
    throw new Error('Raw evidence and previous all-runtime summary do not match');
  }
  const rows = previous.rows.map((row) => {
    if (row.baselineType === 'native-reference') {
      const scenarioId = row.scenario === 'large-photo-upload-webp' ? 'jpeg-webp' : 'png-jpeg';
      const fixture = evidence.fixtures.find((item) => item.id === scenarioId);
      if (!fixture || Object.values(fixture.inputMetadata).some(Boolean)) {
        throw new Error(`Cannot correct native metadata claim without metadata-free input: ${row.scenario}`);
      }
      return { ...row, metadataStripped: null };
    }
    if (row.baselineType !== 'published-package') return row;
    const scenario = SCENARIOS.find((item) => item.id === row.scenario);
    if (!scenario || !['node-wasm', 'browser-worker'].includes(row.runtime)) {
      throw new Error(`Unexpected published summary row: ${row.runtime}/${row.scenario}`);
    }
    return publishedRow(scenario, row.runtime, evidence);
  });
  if (rows.filter((row) => row.baselineType === 'published-package').length !== 4) {
    throw new Error('Previous summary does not contain all four published rows');
  }
  const report = { ...previous,
    artifactPaths: { ...previous.artifactPaths,
      publishedEvidence: path.relative(resolveRoot(), evidencePath) },
    metadataVerification: metadataVerification(evidence, RUNTIME_FILTERS.get(previous.runtimeFilter)),
    aggregation: {
      generatedAt: new Date().toISOString(),
      codeSha: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: resolveRoot(), encoding: 'utf8' }).trim(),
      command: `node test/benchmarks/wasm-upload-comparison.bench.js --reaggregate ${path.relative(resolveRoot(), evidencePath)} --previous-summary ${path.relative(resolveRoot(), previousSummaryPath)}`,
      rawEvidenceSha256: createHash('sha256').update(evidenceBytes).digest('hex'),
      previousSummarySha256: createHash('sha256').update(previousBytes).digest('hex'),
    },
    rows };
  writeReportFiles(report);
  printSummary(rows);
}

function renderMarkdownReport(report) {
  const lines = [
    '# Wasm Upload Benchmark Summary',
    '',
    `Generated: ${report.generatedAt}`,
    '',
    `Published package: ${report.publishedVersion}; measuring code revision: ${report.sourceSha}.`,
    `Command: \`${report.command}\`. Browser/Node details and output hashes: \`${report.artifactPaths.publishedEvidence}\`.`,
    ...(report.aggregation ? [`Aggregation corrected: ${report.aggregation.generatedAt}; code revision: ${report.aggregation.codeSha}.`,
      `Reaggregate: \`${report.aggregation.command}\`. Raw evidence SHA-256: ${report.aggregation.rawEvidenceSha256}.`] : []),
    'Edge isolate remains unmeasured. Optional competitor rows are not performance results.',
    'Rows marked `unavailable` document optional competitor baselines that are not installed or cannot execute in the current Node-local harness.',
    '',
    '| Scenario | Runtime | Package | Type | Status | Browser assets raw | Browser assets gzip | Package dir gzip | Instantiate | First encode | API total | First caller | Warm median | Bytes out | Target hit | Quality | SSIM | PSNR | Metadata stripped | Memory delta | Notes |',
    '|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---:|---|',
  ];

  for (const row of report.rows) {
    const cells = [
      row.scenario,
      row.runtime,
      row.package,
      row.baselineType,
      row.status,
      formatBytes(row.browserBundleBytes),
      formatBytes(row.browserBundleGzipBytes),
      formatBytes(row.packageDirectoryGzipBytes),
      formatMs(row.instantiateMs),
      formatMs(row.firstEncodeMs),
      formatMs(row.totalMs),
      formatMs(row.firstVisibleMs),
      formatMs(row.warmMedianMs),
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

  lines.push('', 'Metadata-only case (`metadata-budget`; separate input from the two upload scenarios):',
    `Raw evidence: \`${report.metadataVerification.rawEvidence}\`; input SHA-256: ${report.metadataVerification.inputSha256}.`,
    '| Runtime | Input EXIF | Input GPS | Input XMP | Input ICC | Output EXIF | Output XMP | Output ICC | Removed | Output SHA-256 |',
    '|---|---|---|---|---|---|---|---|---|---|');
  for (const [runtime, result] of Object.entries(report.metadataVerification.results)) {
    const input = report.metadataVerification.inputMetadata;
    const output = result.outputMetadata;
    lines.push(`| ${runtime} | ${input.exif} | ${input.gpsTag} | ${input.xmp} | ${input.icc} | ${output.exif} | ${output.xmp} | ${output.icc} | ${Object.values(result.removed).every(Boolean)} | ${result.outputSha256} |`);
  }
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

if (require.main === module) {
  const task = getArg('--reaggregate', null)
    ? Promise.resolve().then(() => reaggregate(getArg('--reaggregate'), getArg('--previous-summary')))
    : run(getArg('--runtime', 'all'));
  task.catch((err) => {
    console.error(err.stack || err.message);
    process.exit(1);
  });
}

module.exports = { publishedRow, metadataVerification, renderMarkdownReport, reaggregate, SCENARIOS };
