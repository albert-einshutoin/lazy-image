const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [repository, history] = process.argv.slice(2);
if (!repository || !history) throw Error('Usage: node check-records.cjs <repository> <history>');
const read = file => JSON.parse(fs.readFileSync(file));
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const frozen = read(path.join(history, 'source-manifest.json'));
for (const row of frozen.files) { const f = path.join(repository, row.file); assert.equal(fs.statSync(f).size, row.bytes); assert.equal(hash(f), row.sha256); }
const ledger = read(path.join(history, 'LOG_REDACTIONS.json'));
for (const row of ledger.files) { const f = path.join(history, row.file); assert.equal(fs.statSync(f).size, row.publicBytes); assert.equal(hash(f), row.publicSha256); }
const sameViewport = v => assert.deepEqual([v.width, v.height, v.dpr], [1440, 1000, 1]);
const baselineBinding = read(path.join(history, 'evidence/baseline-binding.json'));
const diagnosticBinding = read(path.join(history, 'evidence/diagnostic-binding.json'));
assert.equal(baselineBinding.evaluationCommit, '9762124a7e04da8bfcd1b1e5c95461b71b060b84');
assert.equal(diagnosticBinding.evaluationCommit, 'e1bfcf508fc3adab080dfe6049c168f9da23ce95');
assert.equal(baselineBinding.collectorSha256, hash(path.join(history, 'collector-baseline-source.txt')));
assert.equal(diagnosticBinding.collectorSha256, hash(path.join(history, 'collector-diagnostic-source.txt')));
const dir = path.join(history, 'evidence/browser');
const expectedNames = [], matrix = read(path.join(history, 'matrix.json'));
let baselineImages = 0, diagnosticImages = 0, sheetProbes = 0;
for (const item of matrix) for (const trial of [1, 2, 3]) {
  const name = `baseline-${item.id}-${trial}.json`; expectedNames.push(name); const row = read(path.join(dir, name));
  assert.equal(row.case, item.id); assert.equal(row.trial, trial); assert.equal(row.pending, false);
  const url = `http://127.0.0.1:8877/${item.path}?trial=${trial}`;
  assert.equal(row.url, url); assert.equal(row.qa.url, url); sameViewport(row.live); sameViewport(row.qa.viewport);
  assert.equal(row.browserZoom.percent, 100); assert.equal(row.qa.decodeError, null); assert.equal(row.qa.images.length, item.images);
  assert.equal(row.htmlSha256, baselineBinding.conditions.find(c => c.id === item.id).htmlSha256);
  const htmlFile = item.id.startsWith('single-') ? path.join(history, 'fixtures', item.path)
    : path.join(repository, 'docs/history/blog-adoption-1.5.0-20261013/preview', item.path);
  assert.equal(row.htmlSha256, hash(htmlFile));
  const refs = [...fs.readFileSync(htmlFile, 'utf8').matchAll(/<img\s[^>]*>/g)].map(match => {
    const src = match[0].match(/src="([^"]+)"/)[1], alt = match[0].match(/alt="([^"]+)"/)[1];
    return [alt, src, new URL(src, url).href];
  });
  assert.deepEqual(row.qa.images.map(image => [image.id, image.src, image.currentSrc]), refs);
  assert.equal(row.collectorSha256, baselineBinding.collectorSha256);
  const oldQa = path.join(repository, 'docs/history/blog-adoption-1.5.0-20261013/preview', item.path.startsWith('quality-single/') ? path.dirname(item.path) : '', 'qa.js');
  assert.equal(row.qaSha256, hash(oldQa));
  for (const image of row.qa.images) { assert.equal(image.complete, true); assert.ok(image.naturalWidth > 0 && image.naturalHeight > 0); }
  baselineImages += row.qa.images.length;
}
for (const [id, page, trials] of [['sheet', 'diagnostic-quality.html', [1, 2, 3]], ['sheet-fresh', 'diagnostic-quality-fresh.html', [1, 2, 3]], ...['broken', 'missing', 'invalid'].map(id => [`control-${id}`, `control-${id}.html`, [1]])]) for (const trial of trials) {
  const name = `diagnostic-${id}-${trial}.json`; expectedNames.push(name); const row = read(path.join(dir, name)), qa = row.qa;
  assert.equal(row.case, id); assert.equal(row.trial, trial); assert.equal(row.pending, false);
  const url = `http://127.0.0.1:8877/${page}?diagnosticTrial=${trial}`;
  assert.equal(row.url, url); assert.equal(qa.urlBefore, url); assert.equal(qa.urlAfter, url);
  sameViewport(row.live); sameViewport(qa.viewportBefore); sameViewport(qa.viewportAfter);
  assert.equal(row.observerSha256, hash(path.join(history, 'diagnose.js')));
  assert.equal(row.observerSha256, diagnosticBinding.files.find(f => f.file === 'diagnose.js').sha256);
  assert.equal(row.htmlSha256, diagnosticBinding.files.find(f => f.file === page).sha256);
  assert.equal(row.collectorSha256, diagnosticBinding.collectorSha256); assert.equal(row.browserZoom.percent, 100);
  const control = id.startsWith('control-'); assert.equal(qa.images.length, control ? 3 : 18); assert.equal(qa.events.length, qa.images.length);
  assert.deepEqual(qa.events.map(e => e.id).sort(), qa.images.map(i => i.id).sort());
  for (const image of qa.images) {
    const bad = image.id === 'control-missing' || image.id === 'control-invalid';
    assert.equal(image.status, bad ? 'error' : 'success'); assert.equal(image.reason, null);
    assert.equal(qa.events.find(e => e.id === image.id).type, bad ? 'error' : 'load');
    if (bad) assert.equal(image.error.name, 'EncodingError');
    else assert.ok(image.after.naturalWidth > 0 && image.after.naturalHeight > 0);
    assert.equal(image.before.src, image.after.src); assert.equal(image.before.currentSrc, image.after.currentSrc);
  }
  assert.equal(qa.probes.length, control ? 3 : 2);
  for (const probe of qa.probes) {
    const missing = probe.url.endsWith('/absent-control.png'), invalid = probe.url.endsWith('/invalid.png');
    assert.equal(probe.httpStatus, missing ? 404 : 200); assert.equal(probe.hashMatches, !missing);
    if (!missing) { assert.equal(probe.sha256, probe.expectedSha256); assert.equal(probe.blobDecode.status, invalid ? 'error' : 'success'); }
    const fixture = probe.url.endsWith('/broken.png') ? 'broken.png' : invalid ? 'invalid.png' : null;
    if (fixture) { const expected = diagnosticBinding.files.find(f => f.file === fixture); assert.equal(probe.sha256, expected.sha256); assert.equal(probe.bytes, expected.bytes); }
    const reference = probe.url.includes('/metadata-1/') ? 'desktop-A/images/metadata-1/image-240.webp' : probe.url.endsWith('/reference/coffee-64.png') ? 'reference/coffee-64.png' : null;
    if (reference) assert.equal(probe.sha256, hash(path.join(repository, 'docs/history/blog-adoption-1.5.0-20261013/preview', reference)));
  }
  if (!control) { diagnosticImages += qa.images.length; sheetProbes += qa.probes.length; }
}
assert.deepEqual(fs.readdirSync(dir).sort(), expectedNames.sort());
const result = { oldFilesUnchanged: frozen.files.length, baselineLoads: 18, baselineImages,
  finalDiagnosticSheetLoads: 6, diagnosticImages, independentSheetBodyAndBlobPasses: sheetProbes,
  finalControlLoads: 3, controlledErrors: 2, truncatedPNGAccepted: true, historicalCause: 'unknown' };
console.log(JSON.stringify(result));
