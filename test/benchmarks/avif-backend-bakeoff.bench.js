const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn, spawnSync } = require('child_process');
const sharp = require('sharp');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');
const { calculateQualityMetrics } = require('../helpers/quality');
const { buildPackageImpactMarkdown, collectBenchmarkPackageImpact } = require('../helpers/package-impact');
const { ImageEngine } = require(resolveRoot('index'));

const DEFAULT_ITERATIONS = 2;
const REQUIRED_CODECS = ['rav1e', 'aom', 'svt'];
const OUTPUT_FILE_BASE = 'avif-backend-bakeoff';
const TEMP_DIR = resolveTemp('benchmarks', OUTPUT_FILE_BASE);
const RSS_SAMPLE_INTERVAL_MS = 25;

const CASES = [
  {
    label: 'large-png-q60',
    source: 'fixture',
    input: resolveFixture('test_4.5MB_5000x5000.png'),
    width: 800,
    quality: 60,
  },
  {
    label: 'photo-jpeg-q75',
    source: 'fixture',
    input: resolveFixture('test_3.2MB_5000x5000.jpg'),
    width: 800,
    quality: 75,
  },
  {
    label: 'alpha-gradient-q60',
    source: 'generated-alpha-gradient',
    width: 384,
    height: 256,
    quality: 60,
    expectAlpha: true,
  },
  {
    label: 'icc-gradient-q60',
    source: 'generated-icc-gradient',
    width: 384,
    height: 256,
    quality: 60,
    expectIcc: true,
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

function parseCodecs(value) {
  return String(value)
    .split(',')
    .map((codec) => codec.trim().toLowerCase())
    .filter(Boolean);
}

function average(values) {
  return values.reduce((sum, value) => sum + value, 0) / Math.max(values.length, 1);
}

function formatNumber(value, digits = 3) {
  if (value == null) return 'n/a';
  if (!Number.isFinite(value)) return 'inf';
  return value.toFixed(digits);
}

function formatBytes(value) {
  if (value == null) return 'n/a';
  return `${(value / 1024).toFixed(1)} KiB`;
}

function formatPercent(value) {
  if (value == null) return 'n/a';
  return `${formatNumber(value, 1)}%`;
}

function firstLine(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean) || null;
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

function lazyImageAvifSpeed(quality) {
  if (quality >= 85) return 6;
  if (quality >= 70) return 7;
  if (quality >= 50) return 8;
  return 9;
}

function lazyImageAvifThreads() {
  const available = typeof os.availableParallelism === 'function' ? os.availableParallelism() : os.cpus().length;
  return Math.max(2, Math.min(8, available || 2));
}

function parseAvailableCodecs(text) {
  const available = new Set();
  for (const codec of REQUIRED_CODECS) {
    const enabledPattern = new RegExp(`\\b${codec}\\b[^\\n]*(?:\\[enc|enc/dec|encoder|encode)`, 'i');
    if (enabledPattern.test(text)) {
      available.add(codec);
    }
  }
  return [...available];
}

function parseCodecVersions(text) {
  const versions = {};
  const regex = /\b(aom|rav1e|svt)\b\s*\[[^\]]*enc[^\]]*\]\s*:?\s*([^,\n)]+)/gi;
  let match;
  while ((match = regex.exec(text)) !== null) {
    versions[match[1].toLowerCase()] = match[2].trim();
  }
  return versions;
}

function probeAvifenc(avifenc, allowMissing) {
  const versionResult = spawnSync(avifenc, ['--version'], { encoding: 'utf8' });

  if (versionResult.error) {
    if (versionResult.error.code === 'ENOENT' && allowMissing) {
      return {
        available: false,
        reason: `${avifenc} was not found`,
      };
    }

    throw new Error(
      [
        `Unable to execute avifenc binary: ${avifenc}`,
        `error: ${versionResult.error.message}`,
        'Install libavif tools and retry, or set AVIFENC=/path/to/avifenc.',
      ].join('\n')
    );
  }

  if (versionResult.status !== 0) {
    throw new Error(
      [
        `Unable to probe avifenc binary: ${avifenc} --version`,
        `exit status: ${versionResult.status}`,
        versionResult.stdout ? `stdout:\n${versionResult.stdout}` : null,
        versionResult.stderr ? `stderr:\n${versionResult.stderr}` : null,
      ]
        .filter(Boolean)
        .join('\n')
    );
  }

  const helpResult = spawnSync(avifenc, ['--help'], { encoding: 'utf8' });
  const versionText = `${versionResult.stdout || ''}\n${versionResult.stderr || ''}`;
  const helpText = `${helpResult.stdout || ''}\n${helpResult.stderr || ''}`;

  return {
    available: true,
    versionLine: firstLine(versionText) || 'unknown',
    helpLine: firstLine(helpText) || 'unknown',
    availableCodecs: parseAvailableCodecs(versionText),
    codecVersions: parseCodecVersions(versionText),
  };
}

function buildRgbGradient(width, height, channels) {
  const data = Buffer.alloc(width * height * channels);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const offset = (y * width + x) * channels;
      data[offset] = Math.round((x / Math.max(width - 1, 1)) * 255);
      data[offset + 1] = Math.round((y / Math.max(height - 1, 1)) * 255);
      data[offset + 2] = Math.round(((x + y) / Math.max(width + height - 2, 1)) * 255);
      if (channels === 4) {
        data[offset + 3] = Math.round((x / Math.max(width - 1, 1)) * 255);
      }
    }
  }
  return data;
}

async function buildReferencePng(testCase) {
  if (testCase.source === 'generated-alpha-gradient') {
    const data = buildRgbGradient(testCase.width, testCase.height, 4);
    return sharp(data, {
      raw: { width: testCase.width, height: testCase.height, channels: 4 },
    })
      .png({ compressionLevel: 0 })
      .toBuffer();
  }

  if (testCase.source === 'generated-icc-gradient') {
    const data = buildRgbGradient(testCase.width, testCase.height, 3);
    return sharp(data, {
      raw: { width: testCase.width, height: testCase.height, channels: 3 },
    })
      .png({ compressionLevel: 0 })
      .withMetadata({ icc: 'srgb' })
      .toBuffer();
  }

  let pipeline = sharp(testCase.input).resize(testCase.width, null, { withoutEnlargement: true }).png({
    compressionLevel: 0,
  });

  if (testCase.expectIcc) {
    pipeline = pipeline.withMetadata({ icc: 'srgb' });
  }

  return pipeline.toBuffer();
}

function sampleRssBytes(pid) {
  if (process.platform === 'win32') return null;

  const result = spawnSync('ps', ['-o', 'rss=', '-p', String(pid)], { encoding: 'utf8' });
  if (result.error || result.status !== 0) return null;

  const rssKb = Number.parseInt(result.stdout.trim(), 10);
  if (!Number.isFinite(rssKb) || rssKb <= 0) return null;
  return rssKb * 1024;
}

function runProcessWithRss(command, args) {
  return new Promise((resolve, reject) => {
    const start = process.hrtime.bigint();
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'] });
    const rssSamples = [];
    let stdout = '';
    let stderr = '';
    let spawnError = null;

    child.stdout.on('data', (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk.toString();
    });
    child.on('error', (error) => {
      spawnError = error;
    });

    const timer = setInterval(() => {
      if (child.pid) {
        const sample = sampleRssBytes(child.pid);
        if (sample != null) rssSamples.push(sample);
      }
    }, RSS_SAMPLE_INTERVAL_MS);

    child.on('close', (code, signal) => {
      clearInterval(timer);
      const totalMs = Number(process.hrtime.bigint() - start) / 1_000_000;
      const peakRss = rssSamples.length ? Math.max(...rssSamples) : null;

      if (spawnError) {
        reject(spawnError);
        return;
      }

      if (code !== 0) {
        reject(
          new Error(
            [
              `${command} ${args.join(' ')} failed`,
              code !== null ? `exit status: ${code}` : null,
              signal ? `signal: ${signal}` : null,
              stdout ? `stdout:\n${stdout}` : null,
              stderr ? `stderr:\n${stderr}` : null,
            ]
              .filter(Boolean)
              .join('\n')
          )
        );
        return;
      }

      resolve({ totalMs, peakRss, stdout, stderr });
    });
  });
}

async function analyzeOutput(data) {
  const metadata = await sharp(data).metadata();
  const traits = {
    hasAlpha: Boolean(metadata.hasAlpha),
    channels: metadata.channels ?? null,
    iccBytes: metadata.icc ? metadata.icc.length : 0,
    width: metadata.width ?? null,
    height: metadata.height ?? null,
    alpha: null,
  };

  if (traits.hasAlpha) {
    const raw = await sharp(data).ensureAlpha().raw().toBuffer();
    let min = 255;
    let max = 0;
    let transparentPixels = 0;

    for (let i = 3; i < raw.length; i += 4) {
      const alpha = raw[i];
      if (alpha < min) min = alpha;
      if (alpha > max) max = alpha;
      if (alpha < 255) transparentPixels += 1;
    }

    traits.alpha = { min, max, transparentPixels };
  }

  return traits;
}

function summarizeBehavior(traits, testCase) {
  return {
    alphaPreserved: testCase.expectAlpha ? Boolean(traits.hasAlpha && traits.alpha?.transparentPixels > 0) : null,
    iccPreserved: testCase.expectIcc ? traits.iccBytes > 0 : null,
  };
}

async function encodeLazyImage(referencePng, testCase) {
  const times = [];
  const totals = [];
  const peakRssValues = [];
  let data = null;
  let metrics = null;

  for (let i = 0; i < testCase.iterations; i += 1) {
    const engine = ImageEngine.from(referencePng);
    if (testCase.expectIcc && typeof engine.keepMetadata === 'function') {
      engine.keepMetadata({ icc: true });
    }

    const result = await engine.toBufferWithMetrics('avif', testCase.quality);
    times.push(result.metrics.encodeMs);
    totals.push(result.metrics.totalMs);
    peakRssValues.push(result.metrics.peakRss);
    data = result.data;
    metrics = result.metrics;
  }

  const [quality, traits] = await Promise.all([calculateQualityMetrics(data, referencePng), analyzeOutput(data)]);

  return {
    label: 'lazy-image-current-rav1e',
    backend: 'rav1e',
    bytes: data.length,
    avgEncodeMs: average(times),
    avgTotalMs: average(totals),
    peakRss: peakRssValues.length ? Math.max(...peakRssValues) : null,
    ssim: quality.ssim,
    psnr: quality.psnr,
    traits,
    behavior: summarizeBehavior(traits, testCase),
    metrics,
  };
}

async function encodeAvifencBackend(referencePngPath, outputPath, testCase, backend, options) {
  const times = [];
  const peakRssValues = [];
  let data = null;

  for (let i = 0; i < testCase.iterations; i += 1) {
    const iterationOutputPath = outputPath.replace(/\.avif$/, `-${i}.avif`);
    fs.rmSync(iterationOutputPath, { force: true });

    const args = [
      '--codec',
      backend,
      '--qcolor',
      String(testCase.quality),
      '--qalpha',
      String(testCase.quality),
      '--speed',
      String(testCase.speed),
      '--jobs',
      String(options.jobs),
      '--yuv',
      '420',
      '--range',
      'full',
      referencePngPath,
      iterationOutputPath,
    ];

    const result = await runProcessWithRss(options.avifenc, args);
    times.push(result.totalMs);
    if (result.peakRss != null) peakRssValues.push(result.peakRss);
    data = fs.readFileSync(iterationOutputPath);
  }

  const [quality, traits] = await Promise.all([calculateQualityMetrics(data, fs.readFileSync(referencePngPath)), analyzeOutput(data)]);

  return {
    label: `avifenc-${backend}`,
    backend,
    bytes: data.length,
    avgEncodeMs: average(times),
    avgTotalMs: average(times),
    peakRss: peakRssValues.length ? Math.max(...peakRssValues) : null,
    ssim: quality.ssim,
    psnr: quality.psnr,
    traits,
    behavior: summarizeBehavior(traits, testCase),
  };
}

function buildDelta(candidate, baseline) {
  return {
    bytesPercent: ((candidate.bytes - baseline.bytes) / Math.max(baseline.bytes, 1)) * 100,
    encodeMsPercent:
      ((candidate.avgEncodeMs - baseline.avgEncodeMs) / Math.max(baseline.avgEncodeMs, 0.001)) * 100,
    totalMsPercent:
      ((candidate.avgTotalMs - baseline.avgTotalMs) / Math.max(baseline.avgTotalMs, 0.001)) * 100,
    peakRssPercent:
      candidate.peakRss == null || baseline.peakRss == null
        ? null
        : ((candidate.peakRss - baseline.peakRss) / Math.max(baseline.peakRss, 1)) * 100,
    ssim: candidate.ssim - baseline.ssim,
    psnr: candidate.psnr - baseline.psnr,
  };
}

async function runCase(testCase, options) {
  const referencePng = await buildReferencePng(testCase);
  const referencePngPath = path.join(TEMP_DIR, `${testCase.label}.png`);
  fs.writeFileSync(referencePngPath, referencePng);

  const lazyCase = {
    ...testCase,
    iterations: options.iterations,
    speed: lazyImageAvifSpeed(testCase.quality),
  };
  const baseline = await encodeLazyImage(referencePng, lazyCase);
  const backends = {};
  const deltas = {};

  for (const backend of options.codecs) {
    const outputPath = path.join(TEMP_DIR, `${testCase.label}-${backend}.avif`);
    const candidate = await encodeAvifencBackend(referencePngPath, outputPath, lazyCase, backend, options);
    backends[backend] = candidate;
    deltas[backend] = buildDelta(candidate, baseline);
  }

  return {
    label: testCase.label,
    source: testCase.source,
    input: testCase.input ? path.relative(resolveRoot(), testCase.input) : testCase.source,
    width: testCase.width,
    height: testCase.height ?? null,
    settings: {
      quality: testCase.quality,
      speed: lazyCase.speed,
      yuv: '420',
      range: 'full',
      qalpha: testCase.quality,
      jobs: String(options.jobs),
      lazyImageThreads: lazyImageAvifThreads(),
    },
    expectations: {
      alpha: Boolean(testCase.expectAlpha),
      icc: Boolean(testCase.expectIcc),
    },
    commands: {
      lazyImage: `ImageEngine.from(referencePng).toBufferWithMetrics('avif', ${testCase.quality})`,
      avifenc: `avifenc --codec <backend> --qcolor ${testCase.quality} --qalpha ${testCase.quality} --speed ${lazyCase.speed} --jobs ${options.jobs} --yuv 420 --range full INPUT OUTPUT`,
    },
    lazyImage: baseline,
    backends,
    deltas,
  };
}

function buildMarkdown(report) {
  const lines = [
    '# AVIF Backend Bake-Off',
    '',
    `Generated: ${report.generatedAt}`,
    `Node: ${report.environment.node}`,
    `Platform: ${report.environment.platform}/${report.environment.arch}`,
    `lazy-image: ${report.versions.lazyImage}`,
    `libavif-sys crate: ${report.versions.libavifSys || 'unknown'}`,
    `rav1e crate: ${report.versions.rav1e || 'unknown'}`,
    `avifenc: ${report.tools.avifenc}`,
    `avifenc version: ${report.versions.avifenc || 'unknown'}`,
    `avifenc available codecs: ${report.tools.availableCodecs.length ? report.tools.availableCodecs.join(', ') : 'unknown'}`,
    `avifenc jobs: ${report.settings.avifencJobs}`,
    `lazy-image AVIF threads: ${report.settings.lazyImageThreads}`,
    `Iterations: ${report.iterations}`,
    '',
    ...buildPackageImpactMarkdown(report.packageImpact),
    '',
    'Command shape: `avifenc --codec <backend> --qcolor <quality> --qalpha <quality> --speed <speed> --jobs <jobs> --yuv 420 --range full INPUT OUTPUT`',
    'Quality/CQ mapping: `qcolor` and `qalpha` use the libavif 0..100 quality scale; codec-specific quantizer mapping remains inside libavif/avifenc.',
    `Tune settings: ${report.settings.tune}`,
    '',
    '| Case | Backend | Bytes | Bytes vs lazy-image | Encode ms | Total ms | Peak RSS | SSIM | SSIM vs lazy-image | PSNR | PSNR vs lazy-image | Alpha OK | ICC OK |',
    '|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|',
  ];

  for (const result of report.results) {
    lines.push(
      [
        result.label,
        result.lazyImage.label,
        result.lazyImage.bytes,
        'baseline',
        formatNumber(result.lazyImage.avgEncodeMs, 1),
        formatNumber(result.lazyImage.avgTotalMs, 1),
        formatBytes(result.lazyImage.peakRss),
        formatNumber(result.lazyImage.ssim, 5),
        'baseline',
        formatNumber(result.lazyImage.psnr, 2),
        'baseline',
        formatBehavior(result.lazyImage.behavior.alphaPreserved),
        formatBehavior(result.lazyImage.behavior.iccPreserved),
      ]
        .join(' | ')
        .replace(/^/, '| ')
        .replace(/$/, ' |')
    );

    for (const backend of report.codecs) {
      const candidate = result.backends[backend];
      const delta = result.deltas[backend];
      lines.push(
        [
          result.label,
          candidate.label,
          candidate.bytes,
          formatPercent(delta.bytesPercent),
          formatNumber(candidate.avgEncodeMs, 1),
          formatNumber(candidate.avgTotalMs, 1),
          formatBytes(candidate.peakRss),
          formatNumber(candidate.ssim, 5),
          formatNumber(delta.ssim, 5),
          formatNumber(candidate.psnr, 2),
          formatNumber(delta.psnr, 2),
          formatBehavior(candidate.behavior.alphaPreserved),
          formatBehavior(candidate.behavior.iccPreserved),
        ]
          .join(' | ')
          .replace(/^/, '| ')
          .replace(/$/, ' |')
      );
    }
  }

  lines.push(
    '',
    'Decision rule: do not switch the production AVIF backend unless a candidate improves output size and objective quality for the target profile without unacceptable encode-time, RSS, package-size, CI, or cross-platform build regressions.'
  );

  return `${lines.join('\n')}\n`;
}

function formatBehavior(value) {
  if (value == null) return 'n/a';
  return value ? 'yes' : 'no';
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
    avifenc: getArg('--avifenc', process.env.AVIFENC || 'avifenc'),
    iterations: parsePositiveInteger(
      getArg('--iterations', process.env.AVIF_BACKEND_BAKEOFF_ITERATIONS || String(DEFAULT_ITERATIONS)),
      '--iterations'
    ),
    jobs: getArg('--jobs', process.env.AVIF_BACKEND_JOBS || String(lazyImageAvifThreads())),
    codecs: parseCodecs(getArg('--codecs', process.env.AVIF_BACKEND_CODECS || REQUIRED_CODECS.join(','))),
    outputDir: path.resolve(getArg('--output-dir', resolveRoot('artifacts', 'benchmark'))),
    allowMissing: hasFlag('--allow-missing-avifenc') || process.env.AVIF_BACKEND_ALLOW_MISSING === '1',
    allowPartial: hasFlag('--allow-missing-backends') || process.env.AVIF_BACKEND_ALLOW_PARTIAL === '1',
  };

  const unknownCodecs = options.codecs.filter((codec) => !REQUIRED_CODECS.includes(codec));
  if (unknownCodecs.length > 0) {
    throw new Error(`Unsupported AVIF backend(s): ${unknownCodecs.join(', ')}`);
  }

  const probe = probeAvifenc(options.avifenc, options.allowMissing);
  if (!probe.available) {
    console.log(`Skipping AVIF backend bake-off: ${probe.reason}`);
    console.log('Set AVIFENC=/path/to/avifenc or install libavif tools to generate benchmark artifacts.');
    return;
  }

  if (probe.availableCodecs.length > 0) {
    const missing = options.codecs.filter((codec) => !probe.availableCodecs.includes(codec));
    if (missing.length > 0 && !options.allowPartial) {
      throw new Error(
        [
          `avifenc is missing required backend(s): ${missing.join(', ')}`,
          `available: ${probe.availableCodecs.join(', ')}`,
          'Use --allow-missing-backends only for partial local experiments, not for publishable benchmark evidence.',
        ].join('\n')
      );
    }
    options.codecs = options.codecs.filter((codec) => probe.availableCodecs.includes(codec));
  }

  if (options.codecs.length === 0) {
    throw new Error('No AVIF backends selected for benchmarking');
  }

  fs.mkdirSync(TEMP_DIR, { recursive: true });

  console.log('=== AVIF Backend Bake-Off ===');
  console.log(`avifenc: ${options.avifenc}`);
  console.log(`avifenc version: ${probe.versionLine}`);
  console.log(`Codecs: ${options.codecs.join(', ')}`);
  console.log(`Iterations: ${options.iterations}`);
  console.log(`Jobs: ${options.jobs}`);
  console.log(`Output directory: ${options.outputDir}\n`);

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
      libavifSys: readCargoLockVersion('libavif-sys'),
      rav1e: readCargoLockVersion('rav1e'),
      avifenc: probe.versionLine,
      avifencCodecs: probe.codecVersions,
    },
    tools: {
      avifenc: options.avifenc,
      avifencProbe: probe.helpLine,
      availableCodecs: probe.availableCodecs,
    },
    settings: {
      yuv: '420',
      range: 'full',
      qualityMode: 'qcolor/qalpha 0..100',
      tune: 'codec/libavif default; pass codec-specific --advanced settings in future evidence PRs if needed',
      avifencJobs: String(options.jobs),
      lazyImageThreads: lazyImageAvifThreads(),
    },
    iterations: options.iterations,
    codecs: options.codecs,
    packageImpact: collectBenchmarkPackageImpact({
      benchmarkTool: 'avifenc',
      candidateBackend: 'libavif backend matrix',
    }),
    commandShape:
      'avifenc --codec <backend> --qcolor <quality> --qalpha <quality> --speed <speed> --jobs <jobs> --yuv 420 --range full INPUT OUTPUT',
    timingNote:
      'lazy-image encodeMs/totalMs come from native ProcessingMetrics; avifenc encodeMs/totalMs measure child-process wall time from the same resized PNG reference input.',
    results,
  };

  const { jsonPath, mdPath } = writeReport(report, options.outputDir);

  console.log('\n| Case | Backend | Bytes | Bytes vs lazy | Encode ms | SSIM vs lazy | Alpha OK | ICC OK |');
  console.log('|---|---|---:|---:|---:|---:|---|---|');
  for (const result of results) {
    console.log(
      [
        result.label,
        result.lazyImage.label,
        result.lazyImage.bytes,
        'baseline',
        formatNumber(result.lazyImage.avgEncodeMs, 1),
        'baseline',
        formatBehavior(result.lazyImage.behavior.alphaPreserved),
        formatBehavior(result.lazyImage.behavior.iccPreserved),
      ]
        .join(' | ')
        .replace(/^/, '| ')
        .replace(/$/, ' |')
    );
    for (const backend of options.codecs) {
      const candidate = result.backends[backend];
      const delta = result.deltas[backend];
      console.log(
        [
          result.label,
          candidate.label,
          candidate.bytes,
          formatPercent(delta.bytesPercent),
          formatNumber(candidate.avgEncodeMs, 1),
          formatNumber(delta.ssim, 5),
          formatBehavior(candidate.behavior.alphaPreserved),
          formatBehavior(candidate.behavior.iccPreserved),
        ]
          .join(' | ')
          .replace(/^/, '| ')
          .replace(/$/, ' |')
      );
    }
  }

  console.log(`\nJSON saved: ${jsonPath}`);
  console.log(`Markdown saved: ${mdPath}`);
  console.log('AVIF backend bake-off completed');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
