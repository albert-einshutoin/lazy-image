'use strict';

// Read saved evidence only. No browser, network, compiler or image decoder runs here.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const root = path.resolve(__dirname, '../../../..');
const read = (file) => JSON.parse(fs.readFileSync(path.join(__dirname, file), 'utf8'));
const sha = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
function pin(base, entry) {
  const file = path.resolve(base, entry.file);
  assert(file.startsWith(`${base}${path.sep}`), `outside evidence root: ${entry.file}`);
  const bytes = fs.readFileSync(file);
  assert.equal(bytes.length, entry.bytes, `bytes: ${entry.file}`);
  assert.equal(sha(bytes), entry.sha256, `sha256: ${entry.file}`);
}
function sumKnown(values) {
  return values.every((v) => Number.isFinite(v) && v >= 0)
    ? values.reduce((a, b) => a + b, 0) : null;
}
const pathname = (url) => new URL(url, 'http://127.0.0.1:8880').pathname;
const normalize = (record) => record.observation || JSON.parse(record.observationText);

function validateObservation(o, c) {
  assert.equal(o.caseId, c.id);
  const expected = { width: c.expected.width, height: c.expected.height, dpr: c.expected.dpr };
  for (const v of [o.started.viewport, o.completed.viewport, o.qa.viewportBefore, o.qa.viewportAfter]) {
    assert.deepEqual(v, expected, `${c.id}: viewport`);
  }
  const page = o.started.page;
  assert.equal(pathname(page), c.page);
  for (const u of [o.completed.page, o.qa.urlBefore, o.qa.urlAfter]) assert.equal(u, page);
  assert.equal(o.pending, false);
  assert.equal(o.observerError, null);
  assert.equal(o.visualViewportScale, 1);
  assert.deepEqual(o.serviceWorker, { supported: true, controller: null, registrations: [], error: null });
  assert.deepEqual(o.qa.probes, []);
  assert.equal(o.qa.images.length, 1);
  const image = o.qa.images[0];
  assert.equal(image.status, 'success', `${c.id}: decode`);
  assert.equal(image.error, null);
  assert.equal(image.reason, null);
  assert.deepEqual(image.before, image.after, `${c.id}: changed image during observation`);
  assert.equal(image.after.complete, true);
  assert(image.after.naturalWidth > 0 && image.after.naturalHeight > 0);
  assert.equal(image.after.rect.width, c.expected.cssWidth);
  assert.equal(image.after.src, c.candidates.find((v) => v.width === 1920).url);
  assert.equal(image.after.srcset, c.mode === 'responsive'
    ? c.candidates.map((v) => `${v.url} ${v.width}w`).join(', ') : null);
  assert.equal(image.after.sizes, c.mode === 'responsive' ? catalog.sizes : '');
  assert(o.qa.events.some((e) => e.type === 'load' && e.src === image.after.currentSrc));
  assert(!o.qa.events.some((e) => e.type === 'error'));
  const urls = new Set(c.candidates.map((v) => new URL(v.url, page).href));
  assert(urls.has(image.after.currentSrc), `${c.id}: selected source`);
  assert.deepEqual(o.imageRequests, o.resources.filter((r) => urls.has(r.name)), `${c.id}: all image entries`);
  assert(o.imageRequests.length > 0);
  assert(o.imageRequests.every((r) => r.initiatorType === 'img' && r.workerStart === 0));
  return image.after;
}

// Verify input bytes before interpreting any saved observation or server record.
const inputs = read('inputs.json');
for (const entry of inputs.files) pin(__dirname, entry);
const provenance = read('provenance.json');
for (const [file, digest] of Object.entries(provenance.sourceHashes)) {
  assert.equal(sha(fs.readFileSync(path.join(root, file))), digest, `source: ${file}`);
}
for (const entry of provenance.copyMap) {
  pin(__dirname, { ...entry, sha256: entry.publicSha256 });
  assert.equal(entry.originalSha256, entry.publicSha256, `copy: ${entry.file}`);
}
const protectedInputs = JSON.parse(fs.readFileSync(path.join(root,
  'docs/history/diagram-srcset-transfer-20261016/source-manifest.json'), 'utf8'));
for (const entry of protectedInputs.files) pin(root, entry);
assert.equal(protectedInputs.files.length, provenance.originalFilesProtected);
assert.equal(sumKnown(protectedInputs.files.map((e) => e.bytes)), provenance.originalBytesProtected);
const catalog = read('catalog.json');
assert.equal(sha(fs.readFileSync(path.join(__dirname, 'catalog.json'))), provenance.catalogSha256);
for (const artifact of catalog.artifacts) pin(root, { ...artifact, file: `${provenance.assetDirectory}/${artifact.path}` });
for (const [url, route] of Object.entries(catalog.routes)) {
  if (route.type.startsWith('text/html')) {
    pin(__dirname, { ...route, file: `html/${path.basename(route.file)}` });
  } else if (route.type === 'image/webp') {
    const artifact = catalog.artifacts.find((a) => `assets/${a.path}` === route.file);
    assert(artifact, url);
    assert.equal(route.sha256, artifact.sha256);
    assert.equal(route.bytes, artifact.bytes);
  } else if (route.type.startsWith('text/javascript')) {
    assert(Object.values(provenance.sourceHashes).includes(route.sha256), `unbound script: ${url}`);
  }
}
const records = inputs.files.filter((e) => e.file.startsWith('raw/'))
  .flatMap((e) => [read(e.file)].flat().map((record) => ({ file: e.file, observation: normalize(record) })));
assert.equal(records.length, 34);
assert.equal(new Set(records.map((r) => r.observation.caseId)).size, 34);
assert.deepEqual(records.map((r) => r.observation.caseId).sort(), catalog.cases.map((c) => c.id).sort());
const log = fs.readFileSync(path.join(__dirname, 'server.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
assert.equal(log[0].catalogSha256, provenance.catalogSha256);
const requests = log.filter((r) => r.event === 'request').sort((a, b) => a.id - b.id);
assert.equal(new Set(requests.map((r) => r.id)).size, requests.length);
const documents = requests.filter((r) => r.responseHeaders['Content-Type']?.startsWith('text/html'));
const cases = records.map(({ file, observation: o }) => {
  const c = catalog.cases.find((entry) => entry.id === o.caseId);
  const image = validateObservation(o, c);
  const urls = new Set(c.candidates.map((a) => a.url));
  const html = documents.filter((r) => r.url === c.page);
  assert.equal(html.length, 1, `${c.id}: unique page load`);
  const start = html[0];
  const end = documents.find((r) => r.id > start.id)?.id ?? Infinity;
  assert.equal(start.sha256, catalog.routes[c.page].sha256);
  assert.equal(start.bodyBytes, catalog.routes[c.page].bytes);
  assert.equal(start.status, 200);
  assert.equal(start.responseCompleted, true);
  assert.equal(start.requestAborted, false);
  assert.equal(start.error, null);
  assert.equal(start.responseHeaders['Cache-Control'], 'no-store');
  const network = requests.filter((r) => r.id > start.id && r.id < end && urls.has(r.url));
  const previous = requests.filter((r) => r.id < start.id && urls.has(r.url));
  if (c.cache !== 'warm') assert.equal(previous.length, 0, `${c.id}: fresh candidate URLs`);
  for (const r of network) {
    const a = c.candidates.find((a) => a.url === r.url);
    assert.equal(r.status, 200);
    assert.equal(r.responseCompleted, true);
    assert.equal(r.requestAborted, false);
    assert.equal(r.error, null);
    assert.equal(r.sha256, a.sha256);
    assert.equal(r.bodyBytes, a.bytes);
    assert.equal(r.responseHeaders['Content-Length'], a.bytes);
    assert.equal(r.responseHeaders.ETag, `"${a.sha256}"`);
    assert.equal(r.responseHeaders['Cache-Control'], 'public, max-age=3600, immutable');
  }
  // Match every occurrence; a Set would silently drop repeated downloads.
  const unmatched = [...network];
  const timings = o.imageRequests.map((r) => {
    const a = c.candidates.find((a) => a.url === pathname(r.name));
    assert.equal(r.responseStatus, 200);
    if (r.encodedBodySize != null) assert.equal(r.encodedBodySize, a.bytes);
    if (r.deliveryType === 'cache' && r.transferSize === 0) {
      const prime = previous.filter((p) => p.url === a.url && p.status === 200 && p.responseCompleted &&
        !p.requestAborted && p.error === null && p.sha256 === a.sha256 && p.bodyBytes === a.bytes &&
        p.responseHeaders['Cache-Control'] === 'public, max-age=3600, immutable');
      assert.equal(c.cache, 'warm');
      assert.equal(prime.length, 1, `${c.id}: prior completed prime`);
      assert.equal(network.filter((n) => n.url === a.url).length, 0);
      return { ...r, width: a.width, serverRequestId: null, primeRequestId: prime[0].id, cacheReuse: true };
    }
    const index = unmatched.findIndex((n) => n.url === a.url);
    assert(index >= 0, `${c.id}: timing without server request`);
    const [matched] = unmatched.splice(index, 1);
    return { ...r, width: a.width, serverRequestId: matched.id, primeRequestId: null, cacheReuse: false };
  });
  assert.equal(unmatched.length, 0, `${c.id}: server request missing from timing`);
  const selected = c.candidates.find((a) => a.url === pathname(image.currentSrc));
  const metrics = Object.fromEntries(['transferSize', 'encodedBodySize', 'decodedBodySize']
    .map((key) => [key, sumKnown(timings.map((r) => r[key]))]));
  return { id: c.id, phase: c.phase, condition: c.condition, mode: c.mode, cache: c.cache, trial: c.trial,
    raw: file, page: o.started.page, htmlSha256: start.sha256, htmlRequestId: start.id,
    viewport: o.started.viewport, cssWidth: image.rect.width, selected: { ...selected, currentSrc: image.currentSrc },
    naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight,
    status: Object.values(metrics).some((v) => v === null) ? 'unknown' : 'complete',
    serverImageRequestIds: network.map((r) => r.id), serverBodyBytes: sumKnown(network.map((r) => r.bodyBytes)),
    resourceTiming: metrics, imageRequests: timings };
});
const attributed = cases.flatMap((c) => c.serverImageRequestIds).sort((a, b) => a - b);
assert.deepEqual(attributed, requests.filter((r) => catalog.routes[r.url]?.type === 'image/webp').map((r) => r.id));
const comparisons = ['d1', 'd2', 'm1', 'm2'].map((condition) => ({ condition,
  trials: [1, 2, 3].map((trial) => {
    const pair = (mode) => cases.find((c) => c.phase === 'formal' && c.cache === 'cold' &&
      c.condition === condition && c.trial === trial && c.mode === mode);
    const responsive = pair('responsive'); const fixed = pair('fixed');
    const reduction = (a, b) => a === null || b === null || b === 0 ? null : (1 - a / b) * 100;
    return { trial, responsive: responsive.id, fixed: fixed.id,
      selectedWidth: responsive.selected.width,
      bodyReductionPercent: reduction(responsive.serverBodyBytes, fixed.serverBodyBytes),
      transferReductionPercent: reduction(responsive.resourceTiming.transferSize, fixed.resourceTiming.transferSize) };
  }) }));
const summary = { issue: 908, executionRevision: provenance.executionRevision,
  scope: 'Saved Chrome observations; server body is OS handoff; Resource Timing is not packet capture; cached encoded sizes are not network bytes.',
  status: cases.every((c) => c.status === 'complete') ? 'complete' : 'unknown',
  counts: { pilot: cases.filter((c) => c.phase === 'pilot').length,
    formalCold: cases.filter((c) => c.phase === 'formal' && c.cache === 'cold').length,
    formalPrime: cases.filter((c) => c.phase === 'formal' && c.cache === 'prime').length,
    formalWarm: cases.filter((c) => c.phase === 'formal' && c.cache === 'warm').length },
  storageBytesAllCandidates: sumKnown(catalog.artifacts.map((a) => a.bytes)), comparisons, cases };

if (process.argv.includes('--self-test')) {
  assert.equal(sumKnown([0, 0]), 0);
  for (const missing of [null, undefined, -1, NaN]) assert.equal(sumKnown([10, missing]), null);
  const invalid = normalize(read('initial/invalid-mobile-pilot.json'));
  assert.throws(() => validateObservation(invalid, catalog.cases.find((c) => c.id === invalid.caseId)), /viewport/);
  const warm = structuredClone(records.find((r) => r.observation.caseId === 'formal-m2-responsive-warm-1').observation);
  warm.imageRequests = warm.imageRequests.filter((r) => r.deliveryType === 'cache');
  assert.throws(() => validateObservation(warm, catalog.cases.find((c) => c.id === warm.caseId)), /all image entries/);
  console.log('PASS: zero/unknown separation, rejected invalid viewport, retained warm additional request; 34 cases and protected inputs verified');
} else if (process.argv.includes('--check')) {
  assert.deepEqual(read('summary.json'), summary);
  console.log(`PASS: ${cases.length} cases, ${attributed.length} network image requests, ${protectedInputs.files.length} protected files; summary matches raw`);
} else {
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
}
