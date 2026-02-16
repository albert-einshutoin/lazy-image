#!/usr/bin/env node

const fs = require('fs');
const path = require('path');

function getArg(name, fallback) {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx + 1 >= process.argv.length) return fallback;
  return process.argv[idx + 1];
}

function loadJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function metricKind(name) {
  const lower = name.toLowerCase();
  if (lower.includes('time')) return 'speed';
  if (lower.includes('size')) return 'size';
  return null;
}

function pctChange(current, baseline) {
  if (baseline === 0) return 0;
  return ((current - baseline) / baseline) * 100;
}

function main() {
  const baselinePath = getArg('--baseline');
  const currentPath = getArg('--current');
  const outputPath = getArg('--output', 'artifacts/benchmark/regression-summary.md');
  const jsonOutputPath = getArg('--json-output', 'artifacts/benchmark/regression-summary.json');
  const mode = getArg('--mode', 'warn');
  const speedThreshold = Number(getArg('--speed-threshold', '15'));
  const sizeThreshold = Number(getArg('--size-threshold', '5'));

  if (!baselinePath || !currentPath) {
    console.error('Usage: check-regression.js --baseline <file> --current <file> [--output <file>] [--mode warn|fail] [--speed-threshold N] [--size-threshold N]');
    process.exit(2);
  }

  const baseline = loadJson(baselinePath);
  const current = loadJson(currentPath);

  const baselineMetrics = baseline.metrics || {};
  const currentMap = new Map(current.map((m) => [m.name, m]));

  const rows = [];
  const regressions = [];

  for (const [name, baseValue] of Object.entries(baselineMetrics)) {
    const cur = currentMap.get(name);
    if (!cur) {
      rows.push({ name, status: 'missing', baseline: baseValue, current: null, changePct: null, thresholdPct: null, kind: metricKind(name) });
      continue;
    }

    const kind = metricKind(name);
    if (!kind) {
      rows.push({ name, status: 'ignored', baseline: baseValue, current: cur.value, changePct: null, thresholdPct: null, kind: null });
      continue;
    }

    const change = pctChange(cur.value, baseValue);
    const threshold = kind === 'speed' ? speedThreshold : sizeThreshold;
    const isRegression = change > threshold;

    rows.push({
      name,
      status: isRegression ? 'regression' : 'ok',
      baseline: baseValue,
      current: cur.value,
      changePct: change,
      thresholdPct: threshold,
      kind,
    });

    if (isRegression) regressions.push(name);
  }

  const newMetrics = [];
  for (const metric of current) {
    if (!(metric.name in baselineMetrics)) {
      newMetrics.push(metric.name);
    }
  }

  const header = [
    '# Benchmark Regression Summary',
    '',
    `- Mode: **${mode}**`,
    `- Speed threshold: **+${speedThreshold}%**`,
    `- Size threshold: **+${sizeThreshold}%**`,
    '',
    '| Metric | Kind | Baseline | Current | Change | Threshold | Status |',
    '|---|---|---:|---:|---:|---:|---|',
  ];

  const lines = rows.map((r) => {
    const change = r.changePct == null ? '-' : `${r.changePct >= 0 ? '+' : ''}${r.changePct.toFixed(2)}%`;
    const threshold = r.thresholdPct == null ? '-' : `+${r.thresholdPct}%`;
    const currentValue = r.current == null ? '-' : r.current;
    return `| ${r.name} | ${r.kind || '-'} | ${r.baseline} | ${currentValue} | ${change} | ${threshold} | ${r.status} |`;
  });

  const footer = [''];
  if (newMetrics.length > 0) {
    footer.push('## New Metrics (not in baseline)');
    for (const name of newMetrics) footer.push(`- ${name}`);
    footer.push('');
  }

  if (regressions.length > 0) {
    footer.push('## Regressions Detected');
    for (const name of regressions) footer.push(`- ${name}`);
  } else {
    footer.push('## Regressions Detected');
    footer.push('- none');
  }

  const markdown = [...header, ...lines, ...footer, ''].join('\n');

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, markdown);

  const summaryJson = {
    mode,
    speedThreshold,
    sizeThreshold,
    regressions,
    rows,
    newMetrics,
    createdAt: new Date().toISOString(),
  };
  fs.mkdirSync(path.dirname(jsonOutputPath), { recursive: true });
  fs.writeFileSync(jsonOutputPath, JSON.stringify(summaryJson, null, 2));

  console.log(markdown);

  if (mode === 'fail' && regressions.length > 0) {
    process.exit(1);
  }
}

main();
