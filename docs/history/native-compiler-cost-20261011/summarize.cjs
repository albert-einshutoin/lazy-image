// Reproduce this evaluation from the retained JSON; no image processing or native imports.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const read = name => JSON.parse(fs.readFileSync(path.join(__dirname, name), 'utf8'));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const engines = ['lazy', 'sharp'];
const formats = ['jpeg', 'webp', 'avif'];
const range = values => values.length ? { min: Math.min(...values), max: Math.max(...values) } : null;

function aggregate(compiler, quality, release) {
  assert.equal(compiler.schemaVersion, 1);
  assert.equal(quality.schemaVersion, 2);
  assert.equal(compiler.environment.revision, quality.environment.revision);
  assert.equal(compiler.corpus.manifestSha256, quality.corpus.manifestSha256);
  assert.deepEqual(compiler.corpus.entries, quality.corpus.entries);
  assert.equal(quality.environment.dirty, false);
  assert.equal(quality.contract.trials, 3);
  const entries = compiler.corpus.entries;
  assert.equal(new Set(entries.map(e => e.id)).size, entries.length);
  assert.deepEqual(compiler.results.map(r => r.id).sort(), entries.map(e => e.id).sort());
  const supported = entries.filter(e => !e.expectations.expectedError);
  assert.deepEqual(quality.rows.map(r => `${r.id}/${r.format}`).sort(),
    supported.flatMap(e => formats.map(f => `${e.id}/${f}`)).sort());

  const compilerCases = compiler.results.map(result => {
    assert.equal(result.trials.length, 3);
    const entry = entries.find(e => e.id === result.id);
    assert.equal(result.category, entry.category);
    assert.deepEqual(result.policy, entry.policy);
    assert.equal(result.source.sha256, entry.sha256);
    for (const trial of result.trials) {
      assert.ok(['pass', 'expected-rejection', 'benchmark-failure'].includes(trial.status));
      assert.equal(trial.source.sha256, entry.sha256);
      assert.deepEqual(trial.policy, entry.policy);
      assert.ok(Number.isFinite(trial.timing.e2eMs));
      if (trial.status === 'expected-rejection') {
        assert.equal(trial.failure.errorCode, entry.expectations.expectedError);
        assert.equal(trial.failure.phase, entry.expectations.expectedPhase);
        assert.equal(trial.checks.rejectedWithoutPublication, true);
      }
    }
    const accepted = !entry.expectations.expectedError && result.trials.every(t => t.accepted === true);
    const expectedRejection = !!entry.expectations.expectedError && result.trials.every(t => t.status === 'expected-rejection');
    return { id: result.id, category: result.category, source: result.source, policy: result.policy, expectations: entry.expectations,
      status: accepted ? 'accepted' : expectedRejection ? 'expected-rejection' : 'processing-or-verification-failure',
      e2eMs: result.e2eMs, sampledPeakRssBytes: Math.max(...result.trials.map(t => t.timing.peakRssBytes)),
      trials: result.trials.map(t => ({ status: t.status, accepted: t.accepted, timing: t.timing,
        strictBudget: t.strictBudget, publication: t.publication, checks: t.checks, failure: t.failure })) };
  });

  const codecCases = quality.rows.map(row => {
    assert.equal(row.category, entries.find(e => e.id === row.id).category);
    const measurements = {};
    for (const engine of engines) {
      const m = row.measurements[engine];
      assert.ok(m && Number.isFinite(m.peakRssBytes));
      assert.deepEqual(m.candidates.map(c => c.quality), quality.contract.qualities);
      for (const candidate of m.candidates) {
        assert.ok(Number.isSafeInteger(candidate.bytes) && candidate.bytes > 0);
        assert.ok(Number.isFinite(candidate.ssim));
        assert.ok(Number.isFinite(candidate.psnrDb) || candidate.psnrDb === 'Infinity');
        assert.equal(candidate.encodeMs.samples.length, 3);
        assert.ok(candidate.encodeMs.samples.every(x => Number.isFinite(x) && x >= 0));
      }
      const matches = row.matches[engine];
      for (const [key, satisfies] of [
        ['qualityMatched', c => c.ssim >= row.target.ssim],
        ['byteMatched', c => c.bytes <= row.target.maxBytes],
      ]) {
        const selected = matches[key];
        if (selected === null) assert.ok(!m.candidates.some(satisfies), `spurious unmet: ${row.id}/${row.format}/${engine}/${key}`);
        else {
          assert.deepEqual(selected, m.candidates.find(c => c.quality === selected.quality));
          assert.ok(satisfies(selected));
          const dominates = key === 'qualityMatched'
            ? c => c.bytes < selected.bytes || (c.bytes === selected.bytes && c.ssim > selected.ssim)
            : c => c.ssim > selected.ssim || (c.ssim === selected.ssim && c.bytes < selected.bytes);
          assert.ok(!m.candidates.some(c => satisfies(c) && dominates(c)), `non-optimal selection: ${row.id}/${row.format}/${engine}/${key}`);
        }
      }
      measurements[engine] = { sampledProcessPeakRssBytes: m.peakRssBytes,
        uniqueCandidateHashes: new Set(m.candidates.map(c => c.sha256)).size,
        candidateByteRange: range(m.candidates.map(c => c.bytes)),
        candidateSsimRange: range(m.candidates.map(c => c.ssim)),
        qualityStatus: matches.qualityMatched === null ? 'quality-floor-unmet' : 'met',
        byteStatus: matches.byteMatched === null ? 'byte-cap-unmet' : 'met',
        qualityMatched: matches.qualityMatched, byteMatched: matches.byteMatched,
        processingFailure: false, measurementUnavailable: false };
    }
    assert.equal(row.measurements.lazy.referenceSha256, row.measurements.sharp.referenceSha256);
    return { id: row.id, category: row.category, format: row.format,
      referenceSha256: row.measurements.lazy.referenceSha256, target: row.target, measurements };
  });

  function summarize(rows) {
    const lazy = r => r.measurements.lazy, sharp = r => r.measurements.sharp;
    const eligible = rows.filter(r => lazy(r).qualityMatched && sharp(r).qualityMatched);
    const byteEligible = rows.filter(r => lazy(r).byteMatched && sharp(r).byteMatched);
    return { cases: rows.length,
      engines: Object.fromEntries(engines.map(e => [e, {
        qualityMet: rows.filter(r => r.measurements[e].qualityStatus === 'met').length,
        qualityUnmet: rows.filter(r => r.measurements[e].qualityStatus === 'quality-floor-unmet').map(r => r.id),
        byteMet: rows.filter(r => r.measurements[e].byteStatus === 'met').length,
        byteUnmet: rows.filter(r => r.measurements[e].byteStatus === 'byte-cap-unmet').map(r => r.id),
        processingFailures: rows.filter(r => r.measurements[e].processingFailure).length,
        measurementsUnavailable: rows.filter(r => r.measurements[e].measurementUnavailable).length,
        sampledRssBytesRange: range(rows.map(r => r.measurements[e].sampledProcessPeakRssBytes)),
        matchedQualityEncodeMedianMsRange: range(rows.flatMap(r => r.measurements[e].qualityMatched ? [r.measurements[e].qualityMatched.encodeMs.p50] : [])),
      }])) ,
      sharedFloorComparison: { eligibleCases: eligible.length,
        lazySmallerCases: eligible.filter(r => lazy(r).qualityMatched.bytes < sharp(r).qualityMatched.bytes).length,
        lazyFasterMedianCases: eligible.filter(r => lazy(r).qualityMatched.encodeMs.p50 < sharp(r).qualityMatched.encodeMs.p50).length,
        lazyToSharpByteRatioRange: range(eligible.map(r => lazy(r).qualityMatched.bytes / sharp(r).qualityMatched.bytes)),
        lazyToSharpEncodeMedianRatioRange: range(eligible.map(r => lazy(r).qualityMatched.encodeMs.p50 / sharp(r).qualityMatched.encodeMs.p50)) },
      sharedCapComparison: { eligibleCases: byteEligible.length,
        lazySsimAtLeastSharpCases: byteEligible.filter(r => lazy(r).byteMatched.ssim >= sharp(r).byteMatched.ssim).length },
      lazyLowerSampledProcessRssCases: rows.filter(r => lazy(r).sampledProcessPeakRssBytes < sharp(r).sampledProcessPeakRssBytes).length };
  }
  const compilerSummary = { cases: compilerCases.length,
    accepted: compilerCases.filter(r => r.status === 'accepted').length,
    supportedCases: supported.length,
    expectedRejections: compilerCases.filter(r => r.status === 'expected-rejection').length,
    failures: compilerCases.filter(r => r.status === 'processing-or-verification-failure').length,
    strictBudgetCases: compilerCases.filter(r => r.status === 'accepted' && r.policy.budgets?.length > 0).length,
    strictBudgetMet: compilerCases.filter(r => r.status === 'accepted' && r.policy.budgets?.length > 0 && r.trials.every(t => t.strictBudget.met)).length,
    measuredTrials: compilerCases.reduce((n, r) => n + r.trials.length, 0),
    measurementsUnavailable: 0 };
  assert.equal(compilerSummary.accepted, release.compiler.accepted);
  assert.equal(compilerSummary.expectedRejections, release.compiler.expectedRejections);
  assert.equal(compilerSummary.strictBudgetMet, release.compiler.strictBudget.met);
  for (const engine of engines) {
    assert.equal(codecCases.filter(r => r.measurements[engine].qualityStatus === 'met').length, release.quality[engine].qualityMet);
    assert.equal(codecCases.filter(r => r.measurements[engine].byteStatus === 'met').length, release.quality[engine].byteMet);
  }
  return { schemaVersion: 1, sourceSha: compiler.environment.revision, packageVersion: compiler.environment.packageVersion,
    sourceKind: 'historical checkout build', compilerSummary, compilerCases,
    qualityContract: quality.contract, codecCases,
    byFormat: Object.fromEntries(formats.map(f => [f, summarize(codecCases.filter(r => r.format === f))])),
    byCategoryAndFormat: Object.fromEntries([...new Set(codecCases.map(r => r.category))].map(category => [category,
      Object.fromEntries(formats.map(f => [f, summarize(codecCases.filter(r => r.category === category && r.format === f))]))])),
    unavailableScopes: ['current published npm1.4.3 timing/quality/RSS', '640/960px constrained quality',
      'population p90', 'exact encoder allocation', 'native binary digest', 'candidate/output pixels not retained in original artifact',
      'compiler full output-set bytes/digests (only budgeted byte counts and large JPEG size retained)',
      'real-user SLA and perceptual acceptance', 'PNG output/profile comparison', 'Wasm'] };
}

function loadEvidence() {
  const provenance = read('provenance.json');
  for (const [name, expected] of Object.entries(provenance.rawHashes)) {
    const bytes = fs.readFileSync(path.join(__dirname, 'raw', name));
    assert.equal(bytes.length, expected.bytes);
    assert.equal(hash(bytes), expected.sha256);
  }
  const release = read('raw/release-evidence.json');
  for (const [name, expected] of Object.entries(release.evidenceHashes)) {
    assert.equal(hash(fs.readFileSync(path.join(__dirname, 'raw', name))), expected);
  }
  return [read('raw/compile-image-corpus.json'), read('raw/quality-matched.json'), release];
}

function renderTables(result) {
  const number = n => Number(n).toFixed(3);
  const mib = n => (n / 1048576).toFixed(2);
  const candidate = c => c ? `${c.bytes} @q${c.quality}` : 'unmet';
  const quality = c => c ? `${c.ssim.toFixed(6)} / ${c.psnrDb === 'Infinity' ? 'Infinity' : number(c.psnrDb)}` : 'unmet';
  const time = c => c ? number(c.encodeMs.p50) : 'floor unmet';
  const lines = ['# Case tables (generated)', '',
    'Source: ' + result.sourceSha + '; checkout native ' + result.packageVersion + '. See EVALUATION.md for interpretation.', '',
    '## Compiler: one combined policy job per case', '',
    '| Case / category | Formats / widths | Outcome | E2E p50 / p90 ms | Sampled RSS MiB |',
    '|---|---|---|---:|---:|'];
  for (const r of result.compilerCases) lines.push(`| ${r.id} / ${r.category} | ${r.policy.formats.join(',')} / ${r.policy.widths.join(',')} | ${r.status} | ${number(r.e2eMs.p50)} / ${number(r.e2eMs.p90)} | ${mib(r.sampledPeakRssBytes)} |`);
  lines.push('', '## Codec: independent quality-floor and byte-cap selections', '',
    'Bytes/time columns use each engine\'s smallest candidate meeting the shared floor. Cap column gives highest SSIM inside the shared cap; PSNR is dB. Unmet is not a processing failure. RSS is the whole grid worker, not per-candidate allocation.', '',
    '| Case / category | Format | Floor / cap B | Floor bytes lazy / sharp | Cap SSIM / PSNR lazy ; sharp | Floor encode p50 ms lazy / sharp | Grid RSS MiB lazy / sharp |',
    '|---|---|---:|---:|---|---:|---:|');
  for (const r of result.codecCases) {
    const l = r.measurements.lazy, s = r.measurements.sharp;
    lines.push(`| ${r.id} / ${r.category} | ${r.format} | ${r.target.ssim.toFixed(6)} / ${r.target.maxBytes} | ${candidate(l.qualityMatched)} / ${candidate(s.qualityMatched)} | ${quality(l.byteMatched)} ; ${quality(s.byteMatched)} | ${time(l.qualityMatched)} / ${time(s.qualityMatched)} | ${mib(l.sampledProcessPeakRssBytes)} / ${mib(s.sampledProcessPeakRssBytes)} |`);
  }
  lines.push('', 'Display rounding does not alter selection targets; raw values/trial samples remain in case-summary.json and raw JSON.', '');
  return lines.join('\n');
}

if (require.main === module) {
  const inputs = loadEvidence();
  const result = aggregate(...inputs);
  if (process.argv.includes('--self-test')) {
    assert.equal(result.compilerSummary.accepted, 13);
    assert.equal(result.compilerSummary.expectedRejections, 2);
    assert.equal(result.compilerSummary.strictBudgetMet, 11);
    assert.equal(result.codecCases.length, 39);
    assert.equal(result.byFormat.jpeg.engines.lazy.qualityMet, 1);
    assert.equal(result.byFormat.webp.engines.lazy.qualityMet, 13);
    assert.equal(result.byFormat.avif.engines.lazy.qualityMet, 11);
    assert.equal(result.byFormat.jpeg.engines.lazy.byteUnmet[0], 'transparent-avif');
    const missing = structuredClone(inputs); missing[1].rows.pop();
    assert.throws(() => aggregate(...missing));
    const wrongReference = structuredClone(inputs); wrongReference[1].rows[0].measurements.lazy.referenceSha256 = 'unbound';
    assert.throws(() => aggregate(...wrongReference));
    const falseUnmet = structuredClone(inputs); falseUnmet[1].rows[0].matches.sharp.qualityMatched = null;
    assert.throws(() => aggregate(...falseUnmet));
    const nonOptimal = structuredClone(inputs);
    const tinyAvif = nonOptimal[1].rows.find(r => r.id === 'transparent-avif' && r.format === 'avif');
    tinyAvif.matches.lazy.qualityMatched = tinyAvif.measurements.lazy.candidates.find(c => c.quality === 10);
    assert.throws(() => aggregate(...nonOptimal), 'a qualifying but larger candidate must be rejected');
    console.log('PASS: denominators, distinct refusal/unmet scopes, references and optimal selection/rejection');
  } else {
    const output = JSON.stringify(result, null, 2) + '\n';
    const outputs = { 'case-summary.json': output, 'case-tables.md': renderTables(result) };
    for (const [name, bytes] of Object.entries(outputs)) {
      if (process.argv.includes('--check')) assert.equal(fs.readFileSync(path.join(__dirname, name), 'utf8'), bytes);
      else fs.writeFileSync(path.join(__dirname, name), bytes);
    }
    console.log(`PASS: ${result.compilerCases.length} compiler cases / ${result.codecCases.length} codec comparisons`);
  }
}
module.exports = { aggregate };
