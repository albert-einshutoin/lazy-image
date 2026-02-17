#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { ImageEngine } = require('../../index');
const { resolveFixture } = require('../../test/helpers/paths');

function getArg(name, fallback = null) {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx + 1 >= process.argv.length) return fallback;
  return process.argv[idx + 1];
}

function parseNumber(value, fallback) {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

function avg(values) {
  if (!values.length) return 0;
  return values.reduce((a, b) => a + b, 0) / values.length;
}

function pctChange(current, baseline) {
  if (!baseline) return 0;
  return ((current - baseline) / baseline) * 100;
}

async function runScenario(input, scenario, warmup, runs) {
  const encodeMs = [];
  const peakRss = [];
  const totalMs = [];

  for (let i = 0; i < warmup + runs; i += 1) {
    const result = await ImageEngine.fromPath(input)
      .resize(800, null)
      .toBufferWithMetrics(scenario.format, scenario.quality);

    if (i >= warmup) {
      encodeMs.push(result.metrics.encodeMs);
      peakRss.push(result.metrics.peakRss / (1024 * 1024));
      totalMs.push(result.metrics.totalMs);
    }
  }

  return {
    name: scenario.name,
    format: scenario.format,
    quality: scenario.quality,
    encodeMsAvg: avg(encodeMs),
    peakRssAvgMb: avg(peakRss),
    totalMsAvg: avg(totalMs),
  };
}

function buildMarkdown(rows, regressions, config) {
  const lines = [];
  lines.push('# Perf Safety Gate Summary');
  lines.push('');
  lines.push(`- Encode threshold: +${config.encodeThresholdPct}%`);
  lines.push(`- Peak RSS threshold: +${config.peakRssThresholdPct}%`);
  lines.push('');
  lines.push('| Scenario | encodeMs (baseline) | encodeMs (current) | peakRssMB (baseline) | peakRssMB (current) | Status |');
  lines.push('|---|---:|---:|---:|---:|---|');

  for (const row of rows) {
    lines.push(
      `| ${row.name} | ${row.baselineEncodeMs.toFixed(2)} | ${row.currentEncodeMs.toFixed(2)} | ${row.baselinePeakRssMb.toFixed(2)} | ${row.currentPeakRssMb.toFixed(2)} | ${row.status} |`
    );
  }

  lines.push('');
  lines.push('## Regressions');
  if (regressions.length === 0) {
    lines.push('- none');
  } else {
    for (const r of regressions) lines.push(`- ${r}`);
  }
  lines.push('');

  return lines.join('\n');
}

async function main() {
  const baselinePath = getArg('--baseline', '.github/benchmarks/perf-safety-baseline.json');
  const outputPath = getArg('--output', 'artifacts/benchmark/perf-safety-summary.md');
  const jsonOutputPath = getArg('--json-output', 'artifacts/benchmark/perf-safety-summary.json');
  const mode = getArg('--mode', 'fail');
  const encodeThresholdPct = parseNumber(getArg('--encode-threshold', '20'), 20);
  const peakRssThresholdPct = parseNumber(getArg('--peak-rss-threshold', '20'), 20);
  const warmup = parseNumber(getArg('--warmup', '1'), 1);
  const runs = parseNumber(getArg('--runs', '3'), 3);

  const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  const scenarios = baseline.scenarios || [];
  const input = resolveFixture('test_4.5MB_5000x5000.png');

  const results = [];
  for (const s of scenarios) {
    results.push(await runScenario(input, s, warmup, runs));
  }

  const rows = [];
  const regressions = [];

  for (const result of results) {
    const base = scenarios.find((s) => s.name === result.name);
    if (!base) continue;

    const encodeChange = pctChange(result.encodeMsAvg, base.encodeMsBaseline);
    const peakRssChange = pctChange(result.peakRssAvgMb, base.peakRssMbBaseline);

    const encodeReg = encodeChange > encodeThresholdPct;
    const rssReg = peakRssChange > peakRssThresholdPct;

    const status = encodeReg || rssReg ? 'regression' : 'ok';

    rows.push({
      name: result.name,
      baselineEncodeMs: base.encodeMsBaseline,
      currentEncodeMs: result.encodeMsAvg,
      baselinePeakRssMb: base.peakRssMbBaseline,
      currentPeakRssMb: result.peakRssAvgMb,
      encodeChangePct: encodeChange,
      peakRssChangePct: peakRssChange,
      status,
    });

    if (encodeReg) regressions.push(`${result.name}: encodeMs +${encodeChange.toFixed(2)}%`);
    if (rssReg) regressions.push(`${result.name}: peakRss +${peakRssChange.toFixed(2)}%`);
  }

  const markdown = buildMarkdown(rows, regressions, { encodeThresholdPct, peakRssThresholdPct });

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, markdown);

  const summary = {
    mode,
    warmup,
    runs,
    encodeThresholdPct,
    peakRssThresholdPct,
    regressions,
    rows,
    createdAt: new Date().toISOString(),
  };

  fs.mkdirSync(path.dirname(jsonOutputPath), { recursive: true });
  fs.writeFileSync(jsonOutputPath, JSON.stringify(summary, null, 2));

  console.log(markdown);

  if (mode === 'fail' && regressions.length > 0) {
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
