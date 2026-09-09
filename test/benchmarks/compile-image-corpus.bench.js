'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const { performance } = require('node:perf_hooks');
const sharp = require('sharp');
const { compileImage } = require('../../index');
const {
  readCorpusManifest,
  sha256,
  verifyCorpusManifest,
} = require('../helpers/benchmark-corpus');
const { resolveRoot, resolveTemp } = require('../helpers/paths');

const MAX_VERIFICATION_READ_BYTES = 64 * 1024;
const MIN_VERIFICATION_ARTIFACT_BYTES = MAX_VERIFICATION_READ_BYTES + 1;
const MANIFEST_PATH = process.env.BENCHMARK_CORPUS_MANIFEST
  || resolveRoot('test/benchmarks/corpus/manifest.json');
const OUTPUT_PATH = process.env.BENCHMARK_OUTPUT_JSON
  || resolveRoot('artifacts/benchmark/compile-image-corpus.json');

function relativeRoot(filePath) {
  return path.relative(resolveRoot(), filePath).replace(/\\/g, '/');
}

function errorSummary(error) {
  return {
    name: error?.name || 'Error',
    message: error?.message || String(error),
    code: error?.code ?? null,
    errorCode: error?.errorCode ?? null,
    phase: error?.phase ?? null,
    category: error?.category ?? error?.errorCategory ?? null,
  };
}

async function measure(run) {
  const rssStartBytes = process.memoryUsage().rss;
  let peakRssBytes = rssStartBytes;
  const sampleRss = () => {
    peakRssBytes = Math.max(peakRssBytes, process.memoryUsage().rss);
  };
  const timer = setInterval(sampleRss, 10);
  const started = performance.now();
  let value;
  let error = null;
  try {
    value = await run();
  } catch (caught) {
    error = caught;
  } finally {
    sampleRss();
    clearInterval(timer);
  }
  return {
    value,
    error,
    timing: {
      e2eMs: Number((performance.now() - started).toFixed(3)),
      rssStartBytes,
      rssEndBytes: process.memoryUsage().rss,
      peakRssBytes,
      rssDeltaBytes: Math.max(0, peakRssBytes - rssStartBytes),
    },
  };
}

async function auditReads(run) {
  const originalOpen = fsp.open;
  const reads = [];
  fsp.open = async function auditedOpen(filePath, ...args) {
    const handle = await originalOpen.call(this, filePath, ...args);
    const read = handle.read.bind(handle);
    handle.read = async (...readArgs) => {
      reads.push({ filePath: String(filePath), requestedBytes: readArgs[2] });
      return read(...readArgs);
    };
    return handle;
  };

  let value;
  let error = null;
  try {
    value = await run();
  } catch (caught) {
    error = caught;
  } finally {
    fsp.open = originalOpen;
  }
  return { value, error, reads };
}

function outputPath(outputDir, relativePath) {
  if (typeof relativePath !== 'string' || relativePath.length === 0
    || path.posix.isAbsolute(relativePath)
    || relativePath.includes('\\')
    || path.posix.normalize(relativePath) !== relativePath) {
    throw new Error('published artifact path is not a normalized relative path');
  }
  const root = path.resolve(outputDir);
  const resolved = path.resolve(root, ...relativePath.split('/'));
  const relative = path.relative(root, resolved);
  if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error('published artifact path escapes its output directory');
  }
  return resolved;
}

function readRegularFile(filePath, label) {
  const stat = fs.lstatSync(filePath);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`${label} must be a regular file`);
  return { stat, data: fs.readFileSync(filePath) };
}

function evaluateBudgets(policy, manifest) {
  const budgets = Array.isArray(policy?.budgets) ? policy.budgets : [];
  if (budgets.length === 0) return null;
  const artifacts = new Map((manifest?.artifacts || []).map((artifact) => [artifact.id, artifact]));
  const checks = budgets.map((budget) => {
    const id = `${budget.width}/${budget.format}`;
    const artifact = artifacts.get(id);
    const met = Boolean(
      artifact
      && artifact.targetBytes === budget.maxBytes
      && Number.isInteger(artifact.bytes)
      && artifact.bytes <= budget.maxBytes,
    );
    return {
      id,
      targetBytes: budget.maxBytes,
      bytes: artifact?.bytes ?? null,
      met,
    };
  });
  return {
    applicable: true,
    met: checks.every((check) => check.met),
    checks,
  };
}

function jpegVerificationEvidence(entry, manifest, reads) {
  if (entry.expectations.jpegVerificationIo !== true) return null;
  const jpegArtifact = manifest?.artifacts?.find((artifact) => (
    artifact.format === 'jpeg' && artifact.bytes >= MIN_VERIFICATION_ARTIFACT_BYTES
  ));
  const outputFile = jpegArtifact?.path || null;
  const outputReads = outputFile
    ? reads.filter((read) => path.basename(read.filePath) === outputFile)
    : [];
  const lengths = outputReads.map((read) => read.requestedBytes);
  const bounded = Boolean(
    jpegArtifact
    && jpegArtifact.bytes >= MIN_VERIFICATION_ARTIFACT_BYTES
    && outputFile
    && lengths.length > 0
    && lengths.every((length) => Number.isSafeInteger(length) && length > 0 && length <= MAX_VERIFICATION_READ_BYTES),
  );
  return {
    applicable: true,
    outputFile,
    artifactBytes: jpegArtifact?.bytes ?? null,
    minimumArtifactBytes: MIN_VERIFICATION_ARTIFACT_BYTES,
    readCount: lengths.length,
    maxRequestedBytes: lengths.length > 0 ? Math.max(...lengths) : null,
    boundedBufferBytes: MAX_VERIFICATION_READ_BYTES,
    passed: bounded,
  };
}

async function verifyPublished({ entry, outputDir, manifest, reads }) {
  const manifestFile = readRegularFile(path.join(outputDir, 'manifest.json'), 'published manifest');
  const publishedManifest = JSON.parse(manifestFile.data.toString('utf8'));
  assert.deepEqual(publishedManifest, manifest, 'published manifest must match compileImage result');
  if (manifest.source.sha256 !== entry.sha256 || manifest.source.bytes !== entry.bytes) {
    throw new Error('published manifest source digest does not match the corpus manifest');
  }
  for (const field of ['exif', 'gps', 'xmp', 'comments', 'unknownAncillary']) {
    if (manifest.metadata?.[field] !== 'stripped') {
      throw new Error(`published metadata policy did not strip ${field}`);
    }
  }

  const files = [
    ...manifest.artifacts,
    ...(manifest.placeholder ? [manifest.placeholder] : []),
  ];
  const expectedNames = new Set(['manifest.json', ...files.map((file) => file.path)]);
  const actualNames = fs.readdirSync(outputDir).sort();
  assert.deepEqual(actualNames, [...expectedNames].sort(), 'published directory must contain only manifest and declared artifacts');

  for (const file of files) {
    const artifactFile = readRegularFile(outputPath(outputDir, file.path), file.path);
    if (artifactFile.data.length !== file.bytes || sha256(artifactFile.data) !== file.sha256) {
      throw new Error(`published artifact digest does not match manifest: ${file.path}`);
    }
  }

  const checks = {
    manifest: true,
    sourceChecksum: true,
    metadataPrivacy: true,
    artifactChecksums: true,
  };
  const budget = evaluateBudgets(entry.policy, manifest);
  if (budget && !budget.met) throw new Error('strict byte budget was not met in the published manifest');
  checks.strictBudget = budget;

  if (entry.expectations.inputFormat && manifest.source.detectedFormat !== entry.expectations.inputFormat) {
    throw new Error(`input format mismatch: expected ${entry.expectations.inputFormat}`);
  }
  if (entry.expectations.orientation === 'absent' && manifest.source.orientation !== null) {
    throw new Error('EXIF Orientation-absent fixture did not remain orientation-absent');
  }
  if (entry.expectations.outputAlpha === true) {
    const avif = manifest.artifacts.find((artifact) => artifact.format === 'avif');
    if (!avif) throw new Error('transparent AVIF artifact is missing');
    const alphaFile = outputPath(outputDir, avif.path);
    const metadata = await sharp(alphaFile).metadata();
    const expected = await sharp(entry.absolutePath)
      .ensureAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true });
    const actual = await sharp(alphaFile)
      .ensureAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true });
    if (expected.info.width !== actual.info.width || expected.info.height !== actual.info.height) {
      throw new Error('transparent AVIF dimensions changed before alpha comparison');
    }
    let alphaMismatchCount = 0;
    let maxAlphaDelta = 0;
    const expectedAlphaValues = new Set();
    const actualAlphaValues = new Set();
    for (let offset = 3; offset < expected.data.length; offset += 4) {
      const expectedAlpha = expected.data[offset];
      const actualAlpha = actual.data[offset];
      expectedAlphaValues.add(expectedAlpha);
      actualAlphaValues.add(actualAlpha);
      const delta = Math.abs(expectedAlpha - actualAlpha);
      maxAlphaDelta = Math.max(maxAlphaDelta, delta);
      if (delta !== 0) alphaMismatchCount += 1;
    }
    checks.transparentAvif = {
      file: avif.path,
      hasAlpha: Boolean(metadata?.hasAlpha),
      expectedAlphaValues: [...expectedAlphaValues].sort((left, right) => left - right),
      actualAlphaValues: [...actualAlphaValues].sort((left, right) => left - right),
      alphaMismatchCount,
      maxAlphaDelta,
    };
    if (!checks.transparentAvif.hasAlpha || alphaMismatchCount !== 0) {
      throw new Error('transparent AVIF output did not preserve input alpha values');
    }
  }

  const jpegIo = jpegVerificationEvidence(entry, manifest, reads);
  if (jpegIo && !jpegIo.passed) throw new Error('JPEG output verification did not use bounded reads');
  if (jpegIo) checks.jpegVerificationIo = jpegIo;

  return { checks, budget, jpegIo };
}

async function runCase(entry, index, runRoot) {
  const outputDir = path.join(runRoot, `case-${index}`);
  let reads = [];
  const budget = evaluateBudgets(entry.policy, null);
  const result = {
    id: entry.id,
    category: entry.category,
    source: {
      path: entry.path,
      bytes: entry.bytes,
      sha256: entry.sha256,
    },
    expectations: entry.expectations,
    policy: entry.policy,
    phaseCoverage: {
      phases: ['preflight', 'processing', 'verification', 'publish'],
      publishImplementationPhase: 'commit',
      observedThrough: null,
    },
    status: 'benchmark-failure',
    accepted: false,
    strictBudget: budget,
    timing: null,
    publication: { committed: false, manifestVerified: false },
    checks: null,
    failure: null,
  };

  const operation = async () => {
    if (entry.expectations.jpegVerificationIo !== true) {
      return compileImage({ inputPath: entry.absolutePath, outputDir, policy: entry.policy });
    }
    const audited = await auditReads(
      () => compileImage({ inputPath: entry.absolutePath, outputDir, policy: entry.policy }),
    );
    reads = audited.reads;
    if (audited.error) {
      audited.error.readAudit = audited.reads;
      throw audited.error;
    }
    return audited.value;
  };

  const measured = await measure(operation);
  result.timing = measured.timing;
  if (measured.error) {
    result.phaseCoverage.observedThrough = measured.error.phase || null;
    result.failure = errorSummary(measured.error);
    if (reads.length > 0) result.readAudit = { readCount: reads.length };
  } else {
    const manifest = measured.value;
    result.phaseCoverage.observedThrough = 'publish';
    try {
      const verified = await verifyPublished({ entry, outputDir, manifest, reads });
      result.status = 'pass';
      result.accepted = true;
      result.strictBudget = verified.budget;
      result.checks = verified.checks;
      result.publication = {
        committed: true,
        manifestVerified: true,
        artifactCount: manifest.artifacts.length + (manifest.placeholder ? 1 : 0),
      };
      if (verified.jpegIo) result.readAudit = verified.jpegIo;
      result.compiler = manifest.compiler;
    } catch (error) {
      result.failure = errorSummary(error);
    }
  }

  fs.rmSync(outputDir, { recursive: true, force: true });
  return result;
}

function buildArtifact(verifiedCorpus, manifestPath, results) {
  const accepted = results.filter((result) => result.accepted).length;
  const budgetResults = results.filter((result) => result.strictBudget?.applicable === true);
  const budgetMet = budgetResults.filter((result) => result.strictBudget.met).length;
  const peakRssBytes = results.reduce((peak, result) => Math.max(peak, result.timing?.peakRssBytes || 0), 0);
  return {
    schemaVersion: 1,
    benchmark: 'compile-image-corpus',
    generatedAt: new Date().toISOString(),
    environment: {
      packageVersion: require('../../package.json').version,
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      parallelism: typeof os.availableParallelism === 'function' ? os.availableParallelism() : null,
    },
    contract: {
      operation: 'compileImage',
      phaseOrder: ['preflight', 'processing', 'verification', 'publish'],
      publishImplementationPhase: 'commit',
      e2eTiming: 'timing.e2eMs covers the compileImage call through its resolved published directory.',
      success: 'compileImage resolved and post-publication manifest, checksum, metadata, and expectation checks passed.',
    },
    corpus: {
      manifestPath: relativeRoot(manifestPath),
      manifestSha256: sha256(fs.readFileSync(manifestPath)),
      name: verifiedCorpus.name,
      license: verifiedCorpus.license,
      entries: verifiedCorpus.entries.map((entry) => ({
        id: entry.id,
        path: entry.path,
        bytes: entry.bytes,
        sha256: entry.sha256,
        license: entry.license,
        category: entry.category,
        source: entry.source,
        expectations: entry.expectations,
        policy: entry.policy,
      })),
    },
    summary: {
      cases: results.length,
      accepted,
      acceptanceRate: accepted / Math.max(results.length, 1),
      strictBudget: {
        cases: budgetResults.length,
        met: budgetMet,
        rate: budgetResults.length > 0 ? budgetMet / budgetResults.length : null,
      },
      peakRssBytes,
      failureReasons: results
        .filter((result) => result.failure)
        .map((result) => ({ id: result.id, ...result.failure })),
    },
    results,
  };
}

async function runBenchmark() {
  const manifest = readCorpusManifest(MANIFEST_PATH);
  const verifiedCorpus = verifyCorpusManifest(manifest, resolveRoot());
  const runRoot = resolveTemp('benchmarks', `compile-image-corpus-${process.pid}-${Date.now()}`);
  fs.mkdirSync(runRoot, { recursive: true });
  const results = [];
  try {
    for (const [index, entry] of verifiedCorpus.entries.entries()) {
      console.log(`▶ ${entry.id}`);
      results.push(await runCase(entry, index, runRoot));
    }
  } finally {
    fs.rmSync(runRoot, { recursive: true, force: true });
  }

  const artifact = buildArtifact(verifiedCorpus, MANIFEST_PATH, results);
  fs.mkdirSync(path.dirname(OUTPUT_PATH), { recursive: true });
  fs.writeFileSync(OUTPUT_PATH, `${JSON.stringify(artifact, null, 2)}\n`);
  console.log(`JSON artifact: ${OUTPUT_PATH}`);
  console.log(`Accepted: ${artifact.summary.accepted}/${artifact.summary.cases}`);
  if (artifact.summary.accepted !== artifact.summary.cases) {
    throw new Error('compile-image corpus benchmark failed after all cases completed');
  }
  return artifact;
}

if (require.main === module) {
  runBenchmark().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}

module.exports = { runBenchmark };
