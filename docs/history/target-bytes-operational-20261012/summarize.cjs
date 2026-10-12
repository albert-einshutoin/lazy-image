'use strict';

const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { sha256, distribution } = require('../../../test/helpers/benchmark-corpus');
const HERE = __dirname;
const DATA = path.join(HERE, 'evidence');

function classify(r) {
  if (r.outcome === 'measurement-failure') return 'measurement-failure';
  if (r.checks.noUnpublishedStaging !== true || (r.error && r.checks.rejectedWithoutNewOutput !== true)) return 'safety-failure';
  if (r.error) {
    if (r.error.errorCode === 'E300') return r.condition.id === 'api-known-max' ? 'known-feasible-false-rejection' : 'budget-unmet';
    if (r.error.errorCode === 'E123') return /processing exceeded \d+ms timeout/.test(r.error.cause || r.error.message) ? 'resource-timeout' : 'resource-cause-unobserved';
    return 'other-error';
  }
  if (!Array.isArray(r.artifacts) || r.artifacts.length === 0) return 'measurement-failure';
  if (r.artifacts.some(a => a.budgetMet === false)) return 'best-effort-unmet';
  const budgets = r.condition.policy?.budgets || (r.condition.budget == null ? [] : [{ maxBytes: r.condition.budget }]);
  if (budgets.some(b => {
    const matches = r.artifacts.filter(a => (b.format === undefined || a.format === b.format) && (b.width === undefined || a.width === b.width));
    return matches.length === 0 || matches.some(a => a.bytes > b.maxBytes);
  })) return 'safety-failure';
  return 'accepted';
}

function summarize(matrix, records) {
  assert.equal(matrix.conditions.length, 18);
  assert.equal(matrix.trials, 3);
  const expected = Object.keys(matrix.variants).flatMap(v => matrix.conditions.flatMap(c => Array.from({ length: matrix.trials }, (_, t) => `${v}/${c.id}/${t}`)));
  const keys = records.map(r => `${r.identity?.variant || r.variant}/${r.condition.id}/${r.trial}`);
  assert.equal(new Set(keys).size, keys.length, 'duplicate trial');
  assert.deepEqual([...keys].sort(), expected.sort(), 'missing or unexpected trial');
  const rows = [];
  for (const c of matrix.conditions) for (const variant of Object.keys(matrix.variants)) {
    const trials = records.filter(r => (r.identity?.variant || r.variant) === variant && r.condition.id === c.id);
    const outcomes = Object.fromEntries([...new Set(trials.map(classify))].map(k => [k, trials.filter(r => classify(r) === k).length]));
    const times = trials.filter(r => Number.isFinite(r.outerMs)).map(r => r.outerMs);
    rows.push({ id: c.id, variant, interface: c.interface, condition: c, outcomes,
      outerMs: times.length ? distribution(times) : null,
      rss: trials.map(r => r.rss || null),
      artifacts: trials.map(r => r.artifacts || null),
      errorCodes: trials.map(r => r.error?.errorCode || null),
      nativeMetrics: trials.map(r => r.nativeMetrics || null),
      nativeTrace: trials.map(r => r.nativeTrace || null),
      successfulComparisonBoundary: 'Compare accepted outputs/quality separately; old faster false-rejections are not equivalent successes.' });
  }
  return { schemaVersion: 1, issue: 887, matrixSha256: sha256(fs.readFileSync(path.join(DATA, 'matrix.json'))),
    primaryTrials: records.length, rows, sampleLimits: 'Single M4, fresh processes, concurrency 1; p90 of 3 is maximum, no p99/SLA claim.' };
}

function verifySaved(matrix, records) {
  const hashes = JSON.parse(fs.readFileSync(path.join(HERE, 'evidence-hashes.json')));
  for (const f of hashes.files) {
    const data = fs.readFileSync(path.join(HERE, f.path));
    assert.equal(data.length, f.bytes); assert.equal(sha256(data), f.sha256);
  }
  assert.equal(matrix.runnerSha256, sha256(fs.readFileSync(path.join(HERE, 'evaluate.cjs'))));
  assert.equal(matrix.fixturesSha256, sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))));
  const builds = JSON.parse(fs.readFileSync(path.join(DATA, 'matched-sdk-build-results.json')));
  assert.equal(builds.length, 2); assert.notEqual(builds[0].nativeSha256, builds[1].nativeSha256);
  const matrixSha = sha256(fs.readFileSync(path.join(DATA, 'matrix.json')));
  for (const r of records) {
    assert.equal(r.workerExitCode, 0, 'worker failure remains unaccepted');
    const build = builds.find(b => b.variant === r.identity.variant);
    assert.equal(build.exitCode, 0); assert.equal(r.identity.sourceSha, build.sourceSha);
    assert.equal(r.identity.nativeSha256, build.nativeSha256);
    assert.equal(r.identity.runnerSha256, matrix.runnerSha256);
    assert.equal(r.execution.matrixSha256, matrixSha);
    assert.deepEqual(r.condition, matrix.conditions.find(c => c.id === r.condition.id));
    assert(Number.isFinite(r.outerMs) && r.outerMs >= 0);
    assert.equal(r.encodeCount, null, 'unobserved encode count must stay unknown');
    if (r.condition.interface === 'cli') {
      assert.equal(r.nativeMetrics, null);
      assert.equal(r.cliProcess.exitCode, r.error ? 1 : 0);
    }
    for (const p of r.postChecks) {
      if (p.kind === 'existing-compiler-output') {
        assert.equal(p.error.errorCode, 'E301'); assert(p.unchangedManifest && p.unchangedArtifacts);
        assert.equal(p.remainingSiblingStaging.length, 0);
      } else if (p.kind === 'strict-existing-file') {
        assert.equal(p.error.errorCode, 'E300'); assert(p.unchangedFile);
      } else if (p.kind === 'best-effort-unmet-control') {
        assert.equal(p.error, null); assert.equal(p.value.budgetMet, false);
      } else throw Error('unknown postcheck');
    }
  }
  const before = builds.find(b => b.variant === 'before'), current = builds.find(b => b.variant === 'current');
  assert.equal(before.sdkRoot, current.sdkRoot); assert.equal(before.cc, current.cc);
  for (const field of ['cargoLockSha256', 'packageLockSha256', 'node', 'cpu', 'platform', 'arch']) {
    assert.equal(new Set(records.map(r => r.identity[field])).size, 1, 'comparison environment changed: ' + field);
  }
  console.log(`PASS: ${records.length} primary records, ${hashes.files.length} file bindings, distinct native source, fixed environment and safety guards`);
}

function selfTest() {
  const base = { condition: { id: 'photo-tight', budget: 100 }, artifacts: [], checks: { noUnpublishedStaging: true, rejectedWithoutNewOutput: true } };
  assert.equal(classify({ ...base, error: { errorCode: 'E300' } }), 'budget-unmet');
  assert.equal(classify({ ...base, error: { errorCode: 'E123', message: 'compiler processing failed', cause: 'processing exceeded 5000ms timeout at encode stage' } }), 'resource-timeout');
  assert.equal(classify({ ...base, error: { errorCode: 'E123', message: 'compiler processing failed' } }), 'resource-cause-unobserved');
  assert.equal(classify({ ...base, error: { errorCode: 'E123', message: 'max pixels exceeded' } }), 'resource-cause-unobserved');
  assert.equal(classify({ ...base, error: null, artifacts: [{ bytes: 90, budgetMet: true }] }), 'accepted');
  assert.equal(classify({ ...base, error: null, artifacts: [{ bytes: 120, budgetMet: false }] }), 'best-effort-unmet');
  assert.equal(classify({ ...base, error: null, artifacts: [{ bytes: 120, budgetMet: true }] }), 'safety-failure');
  assert.equal(classify({ ...base, error: null, artifacts: [] }), 'measurement-failure');
  assert.equal(classify({ ...base, condition: { id: 'api-known-max' }, error: { errorCode: 'E300' } }), 'known-feasible-false-rejection');
  assert.equal(classify({ ...base, outcome: 'measurement-failure', workerExitCode: 0 }), 'measurement-failure');
  assert.equal(classify({ ...base, error: { errorCode: 'E300' }, checks: { noUnpublishedStaging: false, rejectedWithoutNewOutput: false } }), 'safety-failure');
  console.log('PASS: rejection/timeout/unknown/resource/safety/known-false-rejection classifications');
}

if (require.main === module) {
  if (process.argv[2] === '--self-test') selfTest();
  else {
    const matrix = JSON.parse(fs.readFileSync(path.join(DATA, 'matrix.json')));
    const records = fs.readdirSync(path.join(DATA, 'raw')).filter(n => n.endsWith('.json')).map(n => JSON.parse(fs.readFileSync(path.join(DATA, 'raw', n))));
    const result = summarize(matrix, records);
    if (process.argv[2] === '--check') {
      verifySaved(matrix, records);
      assert.deepEqual(JSON.parse(fs.readFileSync(path.join(DATA, 'summary.json'))), result, 'saved summary differs');
      console.log('PASS: deterministic aggregation matches saved summary');
      process.exit(0);
    }
    fs.writeFileSync(path.join(DATA, 'summary.json'), JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify(result.rows.map(r => ({ id: r.id, variant: r.variant, outcomes: r.outcomes, p50: r.outerMs?.p50 ?? null })), null, 2));
  }
}
module.exports = { classify, summarize };
