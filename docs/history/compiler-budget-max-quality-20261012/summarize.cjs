'use strict';
const fs = require('node:fs'), path = require('node:path'), assert = require('node:assert/strict');
const { sha256, distribution, readCorpusManifest, verifyCorpusManifest } = require('../../../test/helpers/benchmark-corpus');
const { resolveRoot } = require('../../../test/helpers/paths');
const { classify } = require('../target-bytes-operational-20261012/summarize.cjs');
const HERE = __dirname, DATA = path.join(HERE, 'evidence');
const read = file => JSON.parse(fs.readFileSync(path.join(DATA, file)));
const records = group => fs.readdirSync(path.join(DATA, group)).filter(f => f.endsWith('.json')).map(f => read(group + '/' + f));

function verifyRecord(r, matrix, group) {
  assert.equal(r.workerExitCode, 0, 'measurement failure is not acceptance');
  assert.equal(r.group, group);
  assert.deepEqual(r.identity, matrix.identity, 'source/native/JS/environment must stay fixed');
  assert.equal(r.execution.matrixSha256, sha256(fs.readFileSync(path.join(DATA, 'matrix.json'))));
  const list = group === 'raw' ? matrix.primary : matrix[group];
  assert.deepEqual(r.condition, list.find(c => c.id === r.condition.id));
  assert.equal(r.encodeCount, null);
  assert(Number.isFinite(r.outerMs) && r.outerMs >= 0);
  assert.equal(r.checks.noUnpublishedStaging, true);
  if (r.error) assert.equal(r.checks.rejectedWithoutNewOutput, true);
  if (r.condition.interface === 'cli') {
    assert.equal(r.cliProcess.signal, null); assert.equal(r.cliProcess.exitCode, r.error ? 1 : 0);
    assert.equal(r.nativeMetrics, null); assert.equal(r.error?.cause ?? null, null);
  }
  if (!r.error) for (const a of r.artifacts) {
    if (r.condition.interface !== 'calibration') {
      assert.equal(a.bytes, a.observed.bytes); assert.equal(a.sha256, a.observed.sha256);
      const budget = r.condition.policy.budgets?.find(b => b.width === a.width && b.format === a.format);
      if (budget) {
        assert(a.bytes <= budget.maxBytes); assert.equal(a.targetBytes, budget.maxBytes);
        assert(Number.isInteger(a.quality) && a.quality >= 30 && a.quality <= (budget.maxQuality ?? 100));
      }
    }
  }
  for (const p of r.postChecks) {
    assert.equal(p.kind, 'existing-compiler-output'); assert.equal(p.error.errorCode, 'E301');
    assert(p.unchangedManifest && p.unchangedArtifacts); assert.deepEqual(p.remainingSiblingStaging, []);
  }
}

function verify(matrix, all) {
  assert.equal(matrix.trials, 3); assert.equal(matrix.concurrency, 1);
  const ledger = JSON.parse(fs.readFileSync(path.join(HERE, 'evidence-hashes.json')));
  for (const f of ledger.files) {
    const bytes = fs.readFileSync(path.join(HERE, f.path)); assert.equal(bytes.length, f.bytes); assert.equal(sha256(bytes), f.sha256);
  }
  assert.equal(matrix.runnerSha256, sha256(fs.readFileSync(path.join(HERE, 'evaluate.cjs'))));
  assert.equal(matrix.fixturesSha256, sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))));
  const fixture = verifyCorpusManifest(readCorpusManifest(path.join(HERE, 'fixtures.json')), resolveRoot());
  const setup = read('setup.json'); assert(setup.commands.every(c => c.exitCode === 0 && c.signal === null));
  assert.equal(matrix.identity.nativeSha256, setup.native.sha256);
  assert.equal(matrix.identity.buildSourceSha, setup.sourceSha); assert.equal(setup.napiCli, '3.9.0');
  for (const file of ['index.js', 'index.d.ts']) assert.equal(matrix.identity.jsSha256[file], setup.generated[file]);
  for (const [group, count, list] of [['raw', 24, matrix.primary], ['regression', 4, matrix.regression], ['proof', 35, matrix.proof]]) {
    const rows = all[group]; assert.equal(rows.length, count);
    const expected = list.flatMap(c => Array.from({ length: group === 'raw' ? 3 : 1 }, (_, t) => c.id + '/' + t)).sort();
    assert.deepEqual(rows.map(r => r.condition.id + '/' + r.trial).sort(), expected, 'no missing/duplicate/unexpected case');
    for (const r of rows) {
      verifyRecord(r, matrix, group);
      const f = fixture.entries.find(f => f.id === r.fixture.id); assert(f);
      for (const key of ['path', 'bytes', 'sha256', 'license', 'source']) assert.deepEqual(r.fixture[key], f[key]);
    }
  }
  assert(all.raw.filter(r => r.condition.cap).every(r => classify(r) === 'accepted'), 'all twelve capped API/CLI trials must be accepted');
  assert.equal(all.raw.flatMap(r => r.postChecks).length, 1);
  const proof = id => { const r = all.proof.find(r => r.condition.id === id); assert(r && !r.error); return r.artifacts[0]; };
  const large = proof('large-q60'), avif = proof('avif-q60');
  assert(large.bytes <= 93782 && avif.bytes <= 12850);
  for (let q = 61; q <= 90; q++) assert(proof('large-q' + q).bytes > 93782, 'a higher in-range JPEG quality fits');
  for (const r of all.raw.filter(r => r.condition.cap)) {
    const p = r.condition.fixture === 'large' ? large : avif;
    assert.equal(r.artifacts[0].quality, 60); assert.equal(r.artifacts[0].bytes, p.bytes); assert.equal(r.artifacts[0].sha256, p.sha256);
  }
  assert.equal(proof('control-q100').bytes, 346); assert(proof('control-q30').bytes > 346);
  const expectedRegression = ['accepted', 'accepted', 'accepted', 'budget-unmet'];
  assert.deepEqual(matrix.regression.map(c => classify(all.regression.find(r => r.condition.id === c.id))), expectedRegression);
  const webp = all.regression.find(r => r.condition.id === 'webp-cap').artifacts[0];
  assert.equal(webp.quality, 60); assert.equal(webp.sha256, proof('webp-q60').sha256);
  assert.equal(all.regression.find(r => r.condition.id === 'jpeg-none').artifacts[0].quality, 85);
  console.log('PASS: 24 primary / 4 regression / 35 independent proofs, bindings, ranges, highest-fit and publication guards');
}

function summarize(matrix, all) {
  return { schemaVersion: 1, issue: 889, sourceSha: matrix.identity.sourceSha, nativeSha256: matrix.identity.nativeSha256,
    matrixSha256: sha256(fs.readFileSync(path.join(DATA, 'matrix.json'))), rows: matrix.primary.map(c => {
      const trials = all.raw.filter(r => r.condition.id === c.id);
      return { id: c.id, interface: c.interface, policy: c.policy,
        outcomes: Object.fromEntries([...new Set(trials.map(classify))].map(k => [k, trials.filter(r => classify(r) === k).length])),
        outerMs: distribution(trials.map(r => r.outerMs)), rss: trials.map(r => r.rss),
        artifacts: trials.map(r => r.artifacts), errors: trials.map(r => r.error), nativeTrace: trials.map(r => r.nativeTrace) };
    }), regression: all.regression.map(r => ({ id: r.condition.id, outcome: classify(r), artifacts: r.artifacts, error: r.error })),
    proofBoundary: 'Independent single-quality work is excluded from primary costs; q61..90 byte-unmet plus fitting q60 prove JPEG highest-fit; fitting AVIF q60 equals its ceiling.' };
}

const matrix = read('matrix.json'), all = Object.fromEntries(['raw', 'regression', 'proof'].map(g => [g, records(g)]));
verify(matrix, all);
const summary = summarize(matrix, all);
if (process.argv[2] === '--check') assert.deepEqual(read('summary.json'), summary, 'aggregation differs');
else fs.writeFileSync(path.join(DATA, 'summary.json'), JSON.stringify(summary, null, 2) + '\n');
console.log(process.argv[2] === '--check' ? 'PASS: deterministic summary matches' : JSON.stringify(summary.rows.map(r => ({ id: r.id, outcomes: r.outcomes, p50: r.outerMs.p50, worst: r.outerMs.worst }))));
