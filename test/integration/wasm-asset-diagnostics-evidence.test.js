const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const snapshot = path.resolve(__dirname, '../../docs/history/wasm-1.3.1/asset-diagnostics-candidate');
const read = (name) => JSON.parse(fs.readFileSync(path.join(snapshot, `${name}-evidence.json`)));
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const positive = read('positive');

assert.equal(positive.verdict, 'PASS');
assert.equal(positive.candidateVersion, '1.3.1');
assert.equal(positive.publishedVersion, undefined);
assert.equal(positive.sourceDirty, false);
assert.equal(positive.packageSource.sourceDirty, false);
assert.equal(positive.packageSource.kind, 'candidate-tarball');
assert.equal(positive.sourceSha, positive.packageSource.sourceCommit);
assert.equal(positive.packages['@alberteinshutoin/lazy-image-wasm'].version, '1.3.1');
assert.match(positive.packages['@alberteinshutoin/lazy-image-wasm'].resolved, /^file:/);
assert.equal(positive.browserResults.totals.imageConversionsPassed, 4);
assert.equal(positive.browserResults.totals.budgetMet, 3);
assert.equal(positive.browserResults.totals.expectedStrictRejections, 1);
assert.match(positive.browserResults.loadMode, /default loader; no wasmModules/);

for (const result of [positive.browserResults.probe, ...positive.browserResults.results]) {
  const file = path.join(snapshot, path.basename(result.output.artifact));
  assert.equal(sha256(fs.readFileSync(file)), result.output.sha256);
  assert.equal(fs.statSync(file).size, result.output.bytes);
}
const metadata = positive.fixtures.find((item) => item.id === 'metadata-budget');
assert.deepEqual(metadata.inputMetadata, { exif: true, gpsTag: true, xmp: true, icc: true });
assert.equal(sha256(fs.readFileSync(path.join(snapshot, 'wasm-metadata-input.jpg'))), metadata.inputSha256);
const bestEffort = positive.browserResults.results.find((item) => item.id === 'metadata-budget').budget.bestEffort;
assert.equal(bestEffort.budgetMet, false);
assert.equal(sha256(fs.readFileSync(path.join(snapshot, path.basename(bestEffort.output.artifact)))),
  bestEffort.output.sha256);

for (const [name, wasm, code, status] of [
  ['mozjpeg_dec', 'mozjpeg_dec.wasm', 'E131', 404],
  ['squoosh_resize_bg', 'squoosh_resize_bg.wasm', 'E503', 404],
  ['webp_enc_simd', 'webp_enc_simd.wasm', 'E300', 404],
  ['corrupt-jpeg', 'mozjpeg_dec.wasm', 'E131', 200],
]) {
  const evidence = read(name);
  assert.equal(evidence.verdict, 'FAIL');
  assert.equal(evidence.diagnosticValidation.status, 'PASS');
  assert.equal(evidence.packageSource.sha256, positive.packageSource.sha256);
  assert.equal(evidence.sourceSha, positive.sourceSha);
  assert.equal(evidence.sourceDirty, false);
  assert.equal(evidence.browserResults.expectedFailure.freshWorker, true);
  assert.equal(evidence.browserResults.expectedFailure.imageProcessingOutcome, 'FAIL');
  const error = evidence.browserResults.expectedFailure.workerError;
  assert.equal(error.code, code);
  assert.equal(error.category, 'CodecError');
  assert.equal(error.recoverable, false);
  assert.match(error.recoveryHint, /DevTools Network/);
  assert.match(error.recoveryHint, /If delivery is valid/);
  assert(evidence.browserResults.expectedFailure.wasmRequests.every((request) =>
    request.url === `/${wasm}` && request.status === status));
  if (name === 'corrupt-jpeg') {
    assert(evidence.browserResults.requests.filter((request) => request.url.endsWith('.wasm'))
      .every((request) => request.status === 200));
    assert.doesNotMatch(error.message, /HTTP 404|missing asset/i);
  }
}

console.log('candidate Chrome evidence and saved image hashes are consistent');
