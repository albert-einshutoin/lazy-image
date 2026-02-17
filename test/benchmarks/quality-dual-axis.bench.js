/**
 * Quality benchmark with dual evaluation axes.
 *
 * Axis A: Similarity to sharp output (compatibility-oriented)
 * Axis B: Similarity to resized source reference (absolute quality-oriented)
 */

const fs = require('fs');
const path = require('path');
const sharp = require('sharp');
const { resolveFixture, resolveTemp, resolveRoot } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));
const { calculateQualityMetrics } = require('../helpers/quality');

const TEST_IMAGE = resolveFixture('test_4.5MB_5000x5000.png');
const OUTPUT_DIR = resolveTemp('benchmarks', 'quality-dual-axis');

if (!fs.existsSync(OUTPUT_DIR)) {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

const CASES = [
  { format: 'jpeg', quality: 80, name: 'JPEG' },
  { format: 'webp', quality: 80, name: 'WebP' },
  { format: 'avif', quality: 60, name: 'AVIF' },
];

function fmt(v, d = 4) {
  if (!Number.isFinite(v)) return 'inf';
  return v.toFixed(d);
}

async function encodeLazy(format, quality) {
  return ImageEngine.fromPath(TEST_IMAGE).resize(800, null).toBuffer(format, quality);
}

async function encodeSharp(format, quality) {
  let p = sharp(TEST_IMAGE).resize(800, null, { withoutEnlargement: true });
  if (format === 'jpeg') p = p.jpeg({ quality, mozjpeg: true });
  else if (format === 'webp') p = p.webp({ quality });
  else if (format === 'avif') p = p.avif({ quality });
  else throw new Error(`Unsupported format: ${format}`);
  return p.toBuffer();
}

async function buildReferencePng() {
  return sharp(TEST_IMAGE)
    .resize(800, null, { withoutEnlargement: true })
    .png({ compressionLevel: 0 })
    .toBuffer();
}

async function run() {
  console.log('=== Dual-Axis Quality Benchmark ===\n');
  console.log(`Input: ${TEST_IMAGE}`);
  console.log('Axis A: lazy-image vs sharp');
  console.log('Axis B: lazy-image/sharp vs resized source reference\n');

  const reference = await buildReferencePng();
  const rows = [];

  for (const c of CASES) {
    console.log(`▶ ${c.name} (q=${c.quality})`);

    const [lazyBuf, sharpBuf] = await Promise.all([
      encodeLazy(c.format, c.quality),
      encodeSharp(c.format, c.quality),
    ]);

    const [axisA, lazyVsRef, sharpVsRef] = await Promise.all([
      calculateQualityMetrics(lazyBuf, sharpBuf),
      calculateQualityMetrics(lazyBuf, reference),
      calculateQualityMetrics(sharpBuf, reference),
    ]);

    rows.push({
      format: c.name,
      quality: c.quality,
      axisA_lazyVsSharp: axisA,
      axisB_lazyVsRef: lazyVsRef,
      axisB_sharpVsRef: sharpVsRef,
      lazySize: lazyBuf.length,
      sharpSize: sharpBuf.length,
    });
  }

  console.log('\n## Axis A: lazy-image vs sharp\n');
  console.log('| Format | SSIM | PSNR(dB) |');
  console.log('|---|---:|---:|');
  for (const r of rows) {
    console.log(`| ${r.format} | ${fmt(r.axisA_lazyVsSharp.ssim)} | ${fmt(r.axisA_lazyVsSharp.psnr, 2)} |`);
  }

  console.log('\n## Axis B: vs resized source reference\n');
  console.log('| Format | lazy SSIM | sharp SSIM | lazy PSNR | sharp PSNR |');
  console.log('|---|---:|---:|---:|---:|');
  for (const r of rows) {
    console.log(
      `| ${r.format} | ${fmt(r.axisB_lazyVsRef.ssim)} | ${fmt(r.axisB_sharpVsRef.ssim)} | ${fmt(
        r.axisB_lazyVsRef.psnr,
        2
      )} | ${fmt(r.axisB_sharpVsRef.psnr, 2)} |`
    );
  }

  console.log('\n## Output Size\n');
  console.log('| Format | lazy(bytes) | sharp(bytes) | diff% |');
  console.log('|---|---:|---:|---:|');
  for (const r of rows) {
    const diff = ((r.lazySize - r.sharpSize) / r.sharpSize) * 100;
    console.log(`| ${r.format} | ${r.lazySize} | ${r.sharpSize} | ${diff.toFixed(1)}% |`);
  }

  const outputPath = process.env.BENCHMARK_OUTPUT_JSON;
  if (outputPath) {
    fs.mkdirSync(path.dirname(outputPath), { recursive: true });
    fs.writeFileSync(outputPath, JSON.stringify(rows, null, 2));
    console.log(`\n📄 JSON saved: ${outputPath}`);
  }

  console.log('\n✅ dual-axis quality benchmark completed');
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
