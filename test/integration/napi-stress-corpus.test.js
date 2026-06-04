/**
 * Regression coverage for the NAPI boundary stress corpus.
 *
 * Keep this as a short smoke test; the long ASan/LSan run is covered by CI.
 */

const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { resolveRoot } = require('../helpers/paths');

const STRESS_SCRIPT = resolveRoot('test/stress/napi-boundary.stress.js');

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

test('stress smoke includes generated malformed fuzz seeds', () => {
  const result = spawnSync(process.execPath, [
    STRESS_SCRIPT,
    '--iterations',
    '1',
    '--malformed-rounds',
    '1',
    '--rss-growth-limit-mb',
    '0',
  ], {
    cwd: resolveRoot(),
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(`${result.stdout}\n${result.stderr}`, /malformedInputs=25/);
});

console.log(`\nTests passed: ${passed}, failed: ${failed}`);
if (failed > 0) {
  process.exit(1);
}
