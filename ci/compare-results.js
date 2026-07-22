#!/usr/bin/env node
'use strict';

const fs = require('node:fs');

const selective = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const full = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const savedMs = Math.max(0, full.wallDurationMs - selective.wallDurationMs);
const reductionPercent = full.wallDurationMs === 0 ? 0 : (savedMs / full.wallDurationMs) * 100;
const missedFailure = selective.failureCount === 0 && full.failureCount > 0;
const comparison = {
  selectiveDurationMs: selective.wallDurationMs,
  fullDurationMs: full.wallDurationMs,
  reductionPercent: Number(reductionPercent.toFixed(2)),
  selectedTargets: selective.selectedTestTargets,
  allTestFiles: full.allTestFiles,
  selectiveFailures: selective.failureCount,
  fullOnlyFailureDetected: missedFailure,
  fallback: selective.fallback,
};
console.log(JSON.stringify(comparison, null, 2));
if (process.env.GITHUB_STEP_SUMMARY) {
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, [
    '## Selective test comparison',
    '',
    '| Metric | Value |',
    '| --- | ---: |',
    `| Selective duration | ${selective.wallDurationMs} ms |`,
    `| Full duration | ${full.wallDurationMs} ms |`,
    `| Reduction | ${comparison.reductionPercent}% |`,
    `| Selected targets | ${selective.selectedTestTargets} |`,
    `| Full test files | ${full.allTestFiles} |`,
    `| Full-only failure | ${missedFailure} |`,
    '',
  ].join('\n'));
}
if (missedFailure) process.exitCode = 1;
