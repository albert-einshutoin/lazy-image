/**
 * Regression coverage for the NAPI boundary stress corpus.
 *
 * Keep this as a short smoke test; the long ASan/LSan run is covered by CI.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resolveRoot, resolveTemp } = require('../helpers/paths');

const STRESS_SCRIPT = resolveRoot('test/stress/napi-boundary.stress.js');
const SUMMARY_DIR = resolveTemp(`napi-stress-corpus-${process.pid}`);

let passed = 0;
let failed = 0;

function report(name, error) {
  if (error) {
    console.log(`not ok - ${name}`);
    console.log(`   Error: ${error.message}`);
    failed += 1;
  } else {
    console.log(`ok - ${name}`);
    passed += 1;
  }
}

function test(name, fn) {
  try {
    fn();
    report(name);
  } catch (error) {
    report(name, error);
  }
}

console.log('=== NAPI Stress Corpus Tests ===\n');

fs.rmSync(SUMMARY_DIR, { recursive: true, force: true });

test('stress smoke includes generated malformed fuzz seeds and writes summaries', () => {
  const summaryJsonPath = path.join(SUMMARY_DIR, 'summary.json');
  const summaryMdPath = path.join(SUMMARY_DIR, 'summary.md');
  const result = spawnSync(process.execPath, [
    STRESS_SCRIPT,
    '--iterations',
    '1',
    '--malformed-rounds',
    '1',
    '--target-bytes-rounds',
    '1',
    '--rss-growth-limit-mb',
    '0',
    '--summary-json',
    summaryJsonPath,
    '--summary-md',
    summaryMdPath,
  ], {
    cwd: resolveRoot(),
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(`${result.stdout}\n${result.stderr}`, /malformedInputs=30/);

  const summary = JSON.parse(fs.readFileSync(summaryJsonPath, 'utf8'));
  assert.equal(summary.tool, 'napi-boundary-stress');
  assert.equal(summary.status, 'passed');
  assert.equal(summary.options.iterations, 1);
  assert.equal(summary.options.malformedRounds, 1);
  assert.equal(summary.options.targetBytesRounds, 1);
  assert.equal(summary.boundaryCoverage.cloneMultiOutputRounds, 1);
  assert.equal(summary.boundaryCoverage.targetBytesSearchRounds, 1);
  assert.equal(summary.boundaryCoverage.targetBytesSearches, 2);
  assert.equal(summary.malformedInputs.total, 30);
  assert.equal(summary.malformedInputs.inline, 5);
  assert.equal(summary.malformedInputs.generatedFuzzSeeds, 25);
  assert.equal(summary.malformedInputs.generatedFuzzFilePathSeeds, 25);
  assert.equal(summary.memory.sampleCount, 2);
  assert.ok(summary.memory.peakRssMb > 0);
  assert.ok(summary.memory.samples.some((sample) => sample.phase === 'iteration-0'));

  const markdown = fs.readFileSync(summaryMdPath, 'utf8');
  assert.match(markdown, /NAPI Boundary Stress Summary/);
  assert.match(markdown, /targetBytesRounds: 1/);
  assert.match(markdown, /cloneMultiOutputRounds: 1/);
  assert.match(markdown, /targetBytesSearches: 2/);
  assert.match(markdown, /generatedFuzzSeeds: 25/);
  assert.match(markdown, /generatedFuzzFilePathSeeds: 25/);
});

console.log(`\nTests passed: ${passed}, failed: ${failed}`);
fs.rmSync(SUMMARY_DIR, { recursive: true, force: true });
if (failed > 0) {
  process.exit(1);
}
