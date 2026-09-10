const assert = require('node:assert/strict');
const fs = require('node:fs');
const { resolveRoot } = require('../helpers/paths');
const { sha256 } = require('../helpers/benchmark-corpus');

function validateEvidence(compiler, quality) {
  const entries = compiler.corpus.entries;
  for (const category of ['photo','ui','illustration','alpha','metadata','hostile']) {
    assert.ok(entries.filter(e=>e.category===category).length >= 2, `missing category coverage: ${category}`);
  }
  assert.equal(quality.corpus.manifestSha256, compiler.corpus.manifestSha256, 'corpus must match');
  assert.equal(compiler.summary.acceptanceRate, 1, 'supported input acceptance regression');
  assert.equal(compiler.summary.strictBudget.rate, 1, 'compiler budget regression');
  assert.equal(compiler.results.length, entries.length);
  for (const entry of entries) {
    const result = compiler.results.find(r=>r.id===entry.id);
    assert.ok(result, `missing compiler case ${entry.id}`);
    assert.equal(result.trials.length, 3);
    for (const trial of result.trials) {
      if (entry.expectations.expectedError) {
        assert.equal(trial.status, 'expected-rejection');
        assert.equal(trial.failure.errorCode, entry.expectations.expectedError);
        assert.equal(trial.checks.rejectedWithoutPublication, true);
      } else {
        assert.equal(trial.accepted, true);
        assert.equal(trial.publication.manifestVerified, true);
        if (trial.strictBudget) assert.equal(trial.strictBudget.met, true);
      }
    }
  }
  const expected = entries.filter(e=>!e.expectations.expectedError).flatMap(e=>['jpeg','webp','avif'].map(f=>`${e.id}/${f}`)).sort();
  assert.deepEqual(quality.rows.map(r=>`${r.id}/${r.format}`).sort(), expected, 'all supported format comparisons required');
}

function writeReport(compiler, quality) {
  validateEvidence(compiler, quality);
  const lines = ['# v1.3.0 release evidence', '',
    `Revision: ${quality.environment.revision}; dirty: ${quality.environment.dirty}.`, '',
    `Compiler: ${compiler.summary.accepted}/${compiler.summary.supportedCases} supported inputs accepted; ${compiler.summary.expectedRejections} expected rejections.`,
    `Strict compiler budgets: ${compiler.summary.strictBudget.met}/${compiler.summary.strictBudget.cases}.`,
    `Compiler E2E ms: p50 ${compiler.summary.e2eMs.p50}, p90 ${compiler.summary.e2eMs.p90}, worst ${compiler.summary.e2eMs.worst}.`, '',
    'Three trials in fresh processes; sampled RSS includes process overhead. Timing is local evidence, not a universal speed ranking.',
    'Quality uses common source pixels composited on white and a finite grid. Unmet is retained; SSIM does not establish human perceptual equivalence.', '',
    '| Case | Format | SSIM-floor bytes lazy / sharp | Byte-cap SSIM lazy / sharp |',
    '|---|---|---:|---:|'];
  for (const row of quality.rows) {
    const values = ['lazy','sharp'].map(e=>row.matches[e]);
    lines.push(`| ${row.id} | ${row.format} | ${values.map(v=>v.qualityMatched?.bytes ?? 'unmet').join(' / ')} | ${values.map(v=>v.byteMatched?.ssim.toFixed(5) ?? 'unmet').join(' / ')} |`);
  }
  const dir = resolveRoot('artifacts/benchmark');
  fs.mkdirSync(dir,{recursive:true});
  fs.writeFileSync(`${dir}/release-evidence.md`,lines.join('\n')+'\n');
  fs.writeFileSync(`${dir}/release-evidence.json`,JSON.stringify({schemaVersion:1,passed:true,
    compiler:compiler.summary,quality:quality.summary,
    evidenceHashes:Object.fromEntries(['compile-image-corpus.json','quality-matched.json'].map(name=>[name,sha256(fs.readFileSync(`${dir}/${name}`))])),
  },null,2)+'\n');
}
if (require.main === module) {
  writeReport(JSON.parse(fs.readFileSync(resolveRoot('artifacts/benchmark/compile-image-corpus.json'))),
    JSON.parse(fs.readFileSync(resolveRoot('artifacts/benchmark/quality-matched.json'))));
}
module.exports = {validateEvidence, writeReport};
