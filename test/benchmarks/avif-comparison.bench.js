const fs = require('fs');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

let sharp;
try {
  sharp = require('sharp');
} catch (e) {
  console.error('❌ sharp is required: npm install sharp');
  process.exit(1);
}

const INPUT = resolveFixture('test_4.5MB_5000x5000.png');
const OUTPUT_DIR = resolveTemp('benchmarks', 'avif-comparison');
const WARMUP = 1;
const ITERATIONS = 5;

if (!fs.existsSync(OUTPUT_DIR)) {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

function fmtBytes(bytes) {
  return `${(bytes / 1024).toFixed(1)} KB`;
}

function avg(values) {
  return values.reduce((a, b) => a + b, 0) / Math.max(values.length, 1);
}

async function runLazy(format, quality) {
  const start = process.hrtime.bigint();
  const data = await ImageEngine.fromPath(INPUT).resize(800, null).toBuffer(format, quality);
  const ms = Number(process.hrtime.bigint() - start) / 1_000_000;
  return { ms, size: data.length };
}

async function runSharp(format, quality) {
  const start = process.hrtime.bigint();
  let p = sharp(INPUT).resize(800, null, { withoutEnlargement: true });

  if (format === 'avif') p = p.avif({ quality });
  else if (format === 'webp') p = p.webp({ quality });
  else if (format === 'jpeg') p = p.jpeg({ quality, mozjpeg: true });
  else throw new Error(`Unsupported format: ${format}`);

  const data = await p.toBuffer();
  const ms = Number(process.hrtime.bigint() - start) / 1_000_000;
  return { ms, size: data.length };
}

async function runCase(c) {
  const lazyTimes = [];
  const sharpTimes = [];
  let lazySize = 0;
  let sharpSize = 0;

  for (let i = 0; i < WARMUP + ITERATIONS; i += 1) {
    const lazy = await runLazy(c.format, c.quality);
    const sharpResult = await runSharp(c.format, c.quality);

    if (i >= WARMUP) {
      lazyTimes.push(lazy.ms);
      sharpTimes.push(sharpResult.ms);
      lazySize = lazy.size;
      sharpSize = sharpResult.size;
    }
  }

  return {
    ...c,
    lazyAvgMs: avg(lazyTimes),
    sharpAvgMs: avg(sharpTimes),
    lazySize,
    sharpSize,
  };
}

async function main() {
  console.log('=== AVIF/WebP/JPEG Benchmark Matrix ===');
  console.log(`Input: ${INPUT}`);
  console.log(`Runs per case: ${ITERATIONS} (warmup ${WARMUP})\n`);

  const cases = [
    { label: 'AVIF q45', format: 'avif', quality: 45, note: 'lower quality / smaller output' },
    { label: 'AVIF q60', format: 'avif', quality: 60, note: 'default quality baseline' },
    { label: 'AVIF q75', format: 'avif', quality: 75, note: 'higher quality / larger output' },
    { label: 'WebP q80', format: 'webp', quality: 80, note: 'WebP baseline' },
    { label: 'JPEG q80', format: 'jpeg', quality: 80, note: 'JPEG baseline' },
  ];

  const results = [];
  for (const c of cases) {
    console.log(`▶ ${c.label}`);
    const r = await runCase(c);
    results.push(r);
  }

  console.log('\n| Case | lazy-image avg ms | sharp avg ms | speed ratio (sharp/lazy) | lazy size | sharp size | size diff | Note |');
  console.log('|---|---:|---:|---:|---:|---:|---:|---|');

  for (const r of results) {
    const ratio = r.sharpAvgMs / Math.max(r.lazyAvgMs, 0.001);
    const sizePct = ((r.lazySize - r.sharpSize) / Math.max(r.sharpSize, 1)) * 100;
    console.log(
      `| ${r.label} | ${r.lazyAvgMs.toFixed(1)} | ${r.sharpAvgMs.toFixed(1)} | ${ratio.toFixed(2)}x | ${fmtBytes(r.lazySize)} | ${fmtBytes(r.sharpSize)} | ${sizePct.toFixed(1)}% | ${r.note} |`
    );
  }

  console.log('\nAVIF quality tiers indicate practical speed/size trade-off bands in current defaults.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
