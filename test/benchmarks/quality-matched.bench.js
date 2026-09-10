const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { performance } = require('node:perf_hooks');
const { resolveRoot } = require('../helpers/paths');

// ssim.js floating-point accumulation can exceed the theoretical range by a few ULPs.
const SSIM_EPSILON = 1e-12;
function selectMatches(candidates, targetSsim, maxBytes) {
  if (!Number.isFinite(targetSsim) || targetSsim < -1 - SSIM_EPSILON || targetSsim > 1 + SSIM_EPSILON
    || !Number.isSafeInteger(maxBytes) || maxBytes < 1) throw new Error('invalid match target');
  for (const c of candidates) {
    if (!Number.isSafeInteger(c.bytes) || c.bytes < 1 || !Number.isFinite(c.ssim)
      || c.ssim < -1 - SSIM_EPSILON || c.ssim > 1 + SSIM_EPSILON) throw new Error('invalid quality candidate');
  }
  return {
    qualityMatched: candidates.filter(c => c.ssim >= targetSsim)
      .sort((a, b) => a.bytes - b.bytes || b.ssim - a.ssim)[0] || null,
    byteMatched: candidates.filter(c => c.bytes <= maxBytes)
      .sort((a, b) => b.ssim - a.ssim || a.bytes - b.bytes)[0] || null,
  };
}

const { readCorpusManifest, verifyCorpusManifest, sha256, distribution, isolatedCase } = require('../helpers/benchmark-corpus');
const manifestPath = process.env.BENCHMARK_CORPUS_MANIFEST || resolveRoot('test/benchmarks/corpus/release-manifest.json');
const qualities = [10, 20, 30, 40, 50, 60, 70, 80, 85, 90, 95, 100];

async function measureCandidates(entry, format, engine) {
  const sharp = require('sharp');
  const { ImageEngine } = require(resolveRoot('index'));
  const { calculateQualityMetrics } = require('../helpers/quality');
  const reference = await sharp(entry.absolutePath).rotate().resize(320, null, { withoutEnlargement: true })
    .flatten({ background: '#ffffff' }).png().toBuffer();
  let peakRssBytes = process.memoryUsage().rss;
  const sample = () => { peakRssBytes = Math.max(peakRssBytes, process.memoryUsage().rss); };
  const timer = setInterval(sample, 10);
  const candidates = [];
  try {
    for (const quality of qualities) {
      const times = [];
      let output;
      for (let trial = 0; trial < 3; trial++) {
        const start = performance.now();
        const encoded = engine === 'lazy'
          ? await ImageEngine.from(reference).toBuffer(format, quality)
          : await sharp(reference).toFormat(format, format === 'jpeg' ? { quality, mozjpeg: true } : { quality }).toBuffer();
        times.push(performance.now() - start);
        if (output && !encoded.equals(output)) throw new Error('non-deterministic repeated encoding');
        output = encoded;
        sample();
      }
      const metrics = await calculateQualityMetrics(output, reference);
      if (!Number.isFinite(metrics.psnr) && metrics.psnr !== Infinity) throw new Error('invalid PSNR');
      candidates.push({ quality, bytes: output.length, sha256: sha256(output), encodeMs: distribution(times),
        ssim: metrics.ssim, psnrDb: metrics.psnr === Infinity ? 'Infinity' : metrics.psnr });
      sample();
    }
  } finally { clearInterval(timer); }
  return { referenceSha256: sha256(reference), peakRssBytes, candidates };
}

async function runBenchmark() {
  const corpus = verifyCorpusManifest(readCorpusManifest(manifestPath));
  const rows = [];
  for (const [index, entry] of corpus.entries.entries()) {
    if (entry.expectations.expectedError) continue;
    for (const format of ['jpeg', 'webp', 'avif']) {
      const row = { id: entry.id, category: entry.category, format, measurements: {} };
      for (const engine of ['lazy', 'sharp']) {
        row.measurements[engine] = isolatedCase(__filename, [index, format, engine]);
      }
      if (row.measurements.lazy.referenceSha256 !== row.measurements.sharp.referenceSha256) throw new Error('reference mismatch');
      const anchor = row.measurements.sharp.candidates.find(c => c.quality === (format === 'avif' ? 60 : 80));
      row.target = { ssim: anchor.ssim, maxBytes: anchor.bytes, source: 'sharp-anchor', quality: anchor.quality };
      row.matches = Object.fromEntries(['lazy', 'sharp'].map(engine =>
        [engine, selectMatches(row.measurements[engine].candidates, anchor.ssim, anchor.bytes)]));
      rows.push(row);
      console.log(`${entry.id}/${format}: lazy quality=${row.matches.lazy.qualityMatched?.quality ?? 'unmet'}, bytes=${row.matches.lazy.byteMatched?.quality ?? 'unmet'}`);
    }
  }
  const artifact = {
    schemaVersion: 2, benchmark: 'quality-matched', generatedAt: new Date().toISOString(),
    environment: {
      revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: resolveRoot(), encoding: 'utf8' }).trim(),
      dirty: execFileSync('git', ['status', '--porcelain'], { cwd: resolveRoot(), encoding: 'utf8' }).trim().length > 0,
      runnerSha256: sha256(fs.readFileSync(__filename)), node: process.version,
      platform: process.platform, arch: process.arch, sharpVersions: require('sharp').versions,
    },
    corpus: { name: corpus.name, manifestSha256: sha256(fs.readFileSync(manifestPath)), license: corpus.license,
      additionalLicenses: corpus.additionalLicenses,
      entries: corpus.entries.map(({ absolutePath, ...entry }) => entry) },
    contract: {
      qualities, trials: 3, search: 'exhaustive over recorded grid; no monotonicity assumption',
      qualityMatched: 'smallest candidate meeting common SSIM floor; null means unmet',
      byteMatched: 'highest SSIM within common byte cap; null means unmet',
      reference: 'sharp auto-orient, resize to at most 320px wide, composite on white, PNG; identical for both encoders',
      limits: 'SSIM is a proxy, not human perceptual equivalence. Alpha composited here; compiler suite tests preservation. Codec-only, not compiler E2E. Hostile inputs excluded and tested separately by compiler suite.',
      timing: 'three sequential encodes per candidate; p50/p90/worst nearest rank; process isolated per fixture/format/engine; no universal speed claim',
      rss: 'sampled every 10ms in isolated fixture/format/engine process, includes reference and quality measurement; not encoder-only RSS',
    },
    summary: Object.fromEntries(['lazy','sharp'].map(engine=>[engine, {
      cases: rows.length, qualityMet: rows.filter(r=>r.matches[engine].qualityMatched).length,
      byteMet: rows.filter(r=>r.matches[engine].byteMatched).length,
      byteTargetRate: rows.filter(r=>r.matches[engine].byteMatched).length / rows.length,
      candidatesPerCase: qualities.length, encodesPerCase: qualities.length * 3,
    }])), rows,
  };
  const outputPath = process.env.BENCHMARK_OUTPUT_JSON || resolveRoot('artifacts/benchmark/quality-matched.json');
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(artifact, null, 2)}\n`);
  console.log(`JSON artifact: ${outputPath}`);
  return artifact;
}

if (require.main === module) {
  const operation = process.argv[2] === '--worker'
    ? measureCandidates(verifyCorpusManifest(readCorpusManifest(manifestPath)).entries[Number(process.argv[3])], process.argv[4], process.argv[5])
      .then(value => process.stdout.write(JSON.stringify(value)))
    : runBenchmark();
  operation.catch(error => { console.error(error); process.exitCode = 1; });
}
module.exports = { selectMatches, runBenchmark };
