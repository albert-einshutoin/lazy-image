const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine, inspectFile } = require(resolveRoot('index'));

const INPUT = resolveFixture('test_4.5MB_5000x5000.png');
const WARMUP = 1;
const ITERATIONS = 6;

const MIN_ESTIMATE_BYTES = 24 * 1024 * 1024;
const DECODE_OVERHEAD_BYTES = 8 * 1024 * 1024;
const FILTER_OVERHEAD_BYTES = 4 * 1024 * 1024;

function bytesForImage(width, height, bpp) {
  return Math.max(1, width) * Math.max(1, height) * bpp;
}

function resizeInside(width, height, targetW, targetH) {
  if (!targetW && !targetH) return { width, height };
  if (targetW && !targetH) return { width: targetW, height: Math.max(1, Math.round((height * targetW) / width)) };
  if (!targetW && targetH) return { width: Math.max(1, Math.round((width * targetH) / height)), height: targetH };

  const rw = targetW / width;
  const rh = targetH / height;
  const scale = Math.min(rw, rh);
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

function estimateWeightedSemaphoreBytes(meta, scenario) {
  let currentW = meta.width;
  let currentH = meta.height;
  let currentBpp = meta.format === 'jpeg' ? 3 : 4;

  let currentBytes = bytesForImage(currentW, currentH, currentBpp);
  let peak = currentBytes + DECODE_OVERHEAD_BYTES;

  for (const op of scenario.ops) {
    let nextW = currentW;
    let nextH = currentH;
    let nextBpp = currentBpp;
    let overhead = FILTER_OVERHEAD_BYTES / 2;

    if (op.type === 'resize') {
      const resized = resizeInside(currentW, currentH, op.width, op.height);
      nextW = resized.width;
      nextH = resized.height;
      overhead = FILTER_OVERHEAD_BYTES;
    } else if (op.type === 'rotate') {
      if (Math.abs(op.degrees) % 180 === 90) {
        nextW = currentH;
        nextH = currentW;
      }
      overhead = FILTER_OVERHEAD_BYTES;
    } else if (op.type === 'grayscale') {
      nextBpp = Math.max(currentBpp, 3);
    }

    const nextBytes = bytesForImage(nextW, nextH, nextBpp);
    peak = Math.max(peak, currentBytes + nextBytes + overhead);
    currentW = nextW;
    currentH = nextH;
    currentBpp = nextBpp;
    currentBytes = nextBytes;
  }

  const outBpp = scenario.format === 'jpeg' ? 3 : 4;
  const outputBytes = bytesForImage(currentW, currentH, outBpp);
  peak = Math.max(peak, currentBytes + outputBytes / 4);

  return Math.max(peak, MIN_ESTIMATE_BYTES);
}

function mb(bytes) {
  return bytes / 1024 / 1024;
}

function avg(arr) {
  return arr.reduce((a, b) => a + b, 0) / Math.max(arr.length, 1);
}

async function runOne(scenario) {
  if (global.gc) global.gc();
  const rssBefore = process.memoryUsage().rss;

  let engine = ImageEngine.fromPath(INPUT);
  for (const op of scenario.ops) {
    if (op.type === 'resize') engine = engine.resize(op.width, op.height, 'inside');
    if (op.type === 'rotate') engine = engine.rotate(op.degrees);
    if (op.type === 'grayscale') engine = engine.grayscale();
  }

  const start = process.hrtime.bigint();
  const { data, metrics } = await engine.toBufferWithMetrics(scenario.format, scenario.quality);
  const elapsedMs = Number(process.hrtime.bigint() - start) / 1_000_000;

  const rssAfter = process.memoryUsage().rss;
  return {
    elapsedMs,
    outBytes: data.length,
    metricsPeakRss: metrics.peakRss,
    rssDelta: Math.max(0, rssAfter - rssBefore),
  };
}

async function runScenario(meta, scenario) {
  const estimate = estimateWeightedSemaphoreBytes(meta, scenario);
  const samples = [];

  for (let i = 0; i < WARMUP + ITERATIONS; i += 1) {
    const one = await runOne(scenario);
    if (i >= WARMUP) samples.push(one);
  }

  const avgRssDelta = avg(samples.map((s) => s.rssDelta));
  const avgMetricsPeak = avg(samples.map((s) => s.metricsPeakRss));
  const avgMs = avg(samples.map((s) => s.elapsedMs));

  return {
    name: scenario.name,
    estimate,
    avgRssDelta,
    avgMetricsPeak,
    avgMs,
    avgOutBytes: avg(samples.map((s) => s.outBytes)),
  };
}

async function main() {
  const meta = inspectFile(INPUT);
  console.log('=== Memory Tracking Benchmark ===');
  console.log(`Input: ${INPUT}`);
  console.log(`Dimensions: ${meta.width}x${meta.height}, format=${meta.format}`);
  console.log(`Runs per scenario: ${ITERATIONS} (warmup ${WARMUP})`);
  console.log('Note: rss delta is process-level approximation; metrics.peakRss is getrusage-based process peak.\n');

  const scenarios = [
    { name: 'jpeg-noops-q80', format: 'jpeg', quality: 80, ops: [] },
    { name: 'webp-resize800-q80', format: 'webp', quality: 80, ops: [{ type: 'resize', width: 800, height: null }] },
    {
      name: 'jpeg-resize-rotate-gray-q75',
      format: 'jpeg',
      quality: 75,
      ops: [
        { type: 'resize', width: 1000, height: null },
        { type: 'rotate', degrees: 90 },
        { type: 'grayscale' },
      ],
    },
  ];

  const results = [];
  for (const s of scenarios) {
    console.log(`▶ ${s.name}`);
    results.push(await runScenario(meta, s));
  }

  console.log('\n| Scenario | Estimated weight (MB) | Avg RSS delta (MB) | Avg metrics.peakRss (MB) | Avg time (ms) | Avg output (KB) | delta/estimate |');
  console.log('|---|---:|---:|---:|---:|---:|---:|');

  for (const r of results) {
    const ratio = r.avgRssDelta / Math.max(r.estimate, 1);
    console.log(
      `| ${r.name} | ${mb(r.estimate).toFixed(1)} | ${mb(r.avgRssDelta).toFixed(1)} | ${mb(r.avgMetricsPeak).toFixed(1)} | ${r.avgMs.toFixed(1)} | ${(r.avgOutBytes / 1024).toFixed(1)} | ${ratio.toFixed(2)} |`
    );
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
