const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { performance } = require('node:perf_hooks');
const { resolveRoot } = require('../helpers/paths');

function selectMatches(candidates, targetSsim, maxBytes) {
  if (!Number.isFinite(targetSsim) || targetSsim < -1 || targetSsim > 1
    || !Number.isSafeInteger(maxBytes) || maxBytes < 1) throw new Error('invalid match target');
  for (const c of candidates) {
    if (!Number.isSafeInteger(c.bytes) || c.bytes < 1 || !Number.isFinite(c.ssim)
      || c.ssim < -1 || c.ssim > 1) throw new Error('invalid quality candidate');
  }
  return {
    qualityMatched: candidates.filter(c => c.ssim >= targetSsim)
      .sort((a, b) => a.bytes - b.bytes || b.ssim - a.ssim)[0] || null,
    byteMatched: candidates.filter(c => c.bytes <= maxBytes)
      .sort((a, b) => b.ssim - a.ssim || a.bytes - b.bytes)[0] || null,
  };
}

async function runBenchmark() {
  const sharp = require('sharp');
  const { ImageEngine } = require(resolveRoot('index'));
  const { calculateQualityMetrics } = require('../helpers/quality');
  const { readCorpusManifest, verifyCorpusManifest, sha256 } = require('../helpers/benchmark-corpus');
  const manifestPath = resolveRoot('test/benchmarks/corpus/manifest.json');
  const corpus = verifyCorpusManifest(readCorpusManifest(manifestPath));
  const qualities = [10, 20, 30, 40, 50, 60, 70, 80, 85, 90, 95, 100];
  const rows = [];
  let peakRssBytes = process.memoryUsage().rss;
  const sampler = setInterval(() => { peakRssBytes = Math.max(peakRssBytes, process.memoryUsage().rss); }, 10);
  try {
    for (const entry of corpus.entries) {
      // Both encoders receive the same pixels; this isolates encoding from resizing/EXIF differences.
      const reference = await sharp(entry.absolutePath).rotate().resize(320, null, { withoutEnlargement: true })
        .flatten({ background: '#ffffff' }).png().toBuffer();
      for (const format of ['jpeg', 'webp', 'avif']) {
        const row = { id: entry.id, format, referenceSha256: sha256(reference), candidates: {} };
        for (const engine of ['lazy', 'sharp']) {
          const candidates = [];
          for (const quality of qualities) {
            const start = performance.now();
            const output = engine === 'lazy'
              ? await ImageEngine.from(reference).toBuffer(format, quality)
              : await sharp(reference).toFormat(format, format === 'jpeg' ? { quality, mozjpeg: true } : { quality }).toBuffer();
            const encodeMs = performance.now() - start;
            const metrics = await calculateQualityMetrics(output, reference);
            if (!Number.isFinite(metrics.psnr) && metrics.psnr !== Infinity) throw new Error('invalid PSNR');
            candidates.push({ quality, bytes: output.length, sha256: sha256(output), encodeMs,
              ssim: metrics.ssim, psnrDb: metrics.psnr === Infinity ? 'Infinity' : metrics.psnr });
            peakRssBytes = Math.max(peakRssBytes, process.memoryUsage().rss);
          }
          row.candidates[engine] = candidates;
        }
        const anchor = row.candidates.sharp.find(c => c.quality === (format === 'avif' ? 60 : 80));
        row.target = { ssim: anchor.ssim, maxBytes: anchor.bytes, source: 'sharp-default-quality-anchor', quality: anchor.quality };
        row.matches = Object.fromEntries(['lazy', 'sharp'].map(engine =>
          [engine, selectMatches(row.candidates[engine], anchor.ssim, anchor.bytes)]));
        rows.push(row);
        console.log(`${entry.id}/${format}: lazy quality=${row.matches.lazy.qualityMatched?.quality ?? 'unmet'}, bytes=${row.matches.lazy.byteMatched?.quality ?? 'unmet'}`);
      }
    }
  } finally {
    clearInterval(sampler);
  }
  const artifact = {
    schemaVersion: 1, benchmark: 'quality-matched', generatedAt: new Date().toISOString(),
    environment: {
      revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: resolveRoot(), encoding: 'utf8' }).trim(),
      dirty: execFileSync('git', ['status', '--porcelain'], { cwd: resolveRoot(), encoding: 'utf8' }).trim().length > 0,
      runnerSha256: sha256(fs.readFileSync(__filename)), node: process.version,
      platform: process.platform, arch: process.arch, sharpVersions: sharp.versions,
    },
    corpus: { name: corpus.name, manifestSha256: sha256(fs.readFileSync(manifestPath)), license: corpus.license,
      entries: corpus.entries.map(({ absolutePath, ...entry }) => entry) },
    contract: {
      qualities, search: 'exhaustive over the recorded grid; no monotonicity assumption',
      qualityMatched: 'smallest candidate meeting the common SSIM floor; null means unmet',
      byteMatched: 'highest SSIM candidate within the common byte cap; null means unmet',
      reference: 'sharp auto-orient, resize to at most 320px wide, composite on white, PNG; shared by both encoders',
      limits: 'SSIM is a proxy, not human perceptual equivalence. Alpha is composited, not tested. Codec-only, not compiler E2E. Three regression fixtures are not a representative corpus.',
      timing: 'single cold/warm mixed sequential sample per candidate; encode/decode input included, metrics excluded; not a performance ranking',
      rss: 'whole-process peak sampled every 10ms including reference and quality metrics; not per-codec memory',
    },
    peakRssBytes, rows,
  };
  const outputPath = process.env.BENCHMARK_OUTPUT_JSON || resolveRoot('artifacts/benchmark/quality-matched.json');
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(artifact, null, 2)}\n`);
  console.log(`JSON artifact: ${outputPath}`);
  return artifact;
}

if (require.main === module) runBenchmark().catch(error => { console.error(error); process.exitCode = 1; });
module.exports = { selectMatches, runBenchmark };
