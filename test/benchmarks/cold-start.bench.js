const { spawnSync } = require('child_process');
const { resolveFixture, resolveRoot } = require('../helpers/paths');

const ROOT = resolveRoot();
const INPUT = resolveFixture('test_100KB_1188x1188.png');
const RUNS = 12;

function runColdStart() {
  const script = `
const t0 = process.hrtime.bigint();
const mod = require('./index');
const t1 = process.hrtime.bigint();
(async () => {
  const startOp = process.hrtime.bigint();
  const out = await mod.ImageEngine.fromPath(${JSON.stringify(INPUT)})
    .resize(800, null)
    .toBuffer('jpeg', 80);
  const t2 = process.hrtime.bigint();
  const requireMs = Number(t1 - t0) / 1_000_000;
  const firstOpMs = Number(t2 - startOp) / 1_000_000;
  const totalMs = Number(t2 - t0) / 1_000_000;
  process.stdout.write(JSON.stringify({ requireMs, firstOpMs, totalMs, bytes: out.length }));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});`;

  const proc = spawnSync(process.execPath, ['-e', script], {
    cwd: ROOT,
    encoding: 'utf8',
    env: process.env,
  });

  if (proc.status !== 0) {
    throw new Error(proc.stderr || `child failed with status ${proc.status}`);
  }

  return JSON.parse(proc.stdout.trim());
}

function avg(values) {
  return values.reduce((a, b) => a + b, 0) / Math.max(values.length, 1);
}

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length));
  return sorted[idx];
}

function fmtMs(v) {
  return `${v.toFixed(1)}ms`;
}

function main() {
  console.log('=== Cold Start Benchmark (serverless-oriented) ===');
  console.log(`Input: ${INPUT}`);
  console.log(`Runs: ${RUNS} cold Node.js processes\n`);

  const samples = [];
  for (let i = 0; i < RUNS; i += 1) {
    const one = runColdStart();
    samples.push(one);
    console.log(`run ${String(i + 1).padStart(2, '0')}: require=${fmtMs(one.requireMs)}, firstOp=${fmtMs(one.firstOpMs)}, total=${fmtMs(one.totalMs)}`);
  }

  const requireArr = samples.map((s) => s.requireMs);
  const opArr = samples.map((s) => s.firstOpMs);
  const totalArr = samples.map((s) => s.totalMs);

  console.log('\n| Metric | Avg | p50 | p95 |');
  console.log('|---|---:|---:|---:|');
  console.log(`| require() time | ${fmtMs(avg(requireArr))} | ${fmtMs(percentile(requireArr, 50))} | ${fmtMs(percentile(requireArr, 95))} |`);
  console.log(`| first toBuffer() time | ${fmtMs(avg(opArr))} | ${fmtMs(percentile(opArr, 50))} | ${fmtMs(percentile(opArr, 95))} |`);
  console.log(`| total cold start | ${fmtMs(avg(totalArr))} | ${fmtMs(percentile(totalArr, 50))} | ${fmtMs(percentile(totalArr, 95))} |`);
}

main();
