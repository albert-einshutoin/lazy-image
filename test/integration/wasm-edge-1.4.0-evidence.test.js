const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');

const snapshot = path.resolve(__dirname, '../../docs/history/v1.4.0/edge-workerd');
const read = (name) => JSON.parse(fs.readFileSync(path.join(snapshot, `${name}-evidence.json`)));
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');

async function main() {
  const positive = read('positive');
  const version = JSON.parse(fs.readFileSync(path.join(snapshot, 'version-check.json')));
  assert.equal(positive.verdict, 'PASS');
  assert.equal(positive.publishedVersion, '1.4.0');
  assert.equal(positive.sourceDirty, false);
  assert.equal(positive.packageSource.kind, 'published-registry');
  assert.equal(version.VERSION, '1.4.0');
  assert.equal(version.integrity, positive.packages['@alberteinshutoin/lazy-image-wasm'].integrity);
  assert.equal(positive.edgeResults.status, 'PASS');
  assert.equal(positive.edgeResults.compatibilityDate, '2026-09-24');
  assert.deepEqual(positive.edgeTotals, { imageConversionsPassed: 4, imageConversionsAttempted: 4,
    budgetMet: 3, budgetAttempts: 4, expectedStrictRejections: 1, strictAttempts: 1 });
  const summary = JSON.parse(fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.json')));
  const { renderMarkdownReport } = require('../benchmarks/wasm-upload-comparison.bench');
  assert.equal(renderMarkdownReport(summary),
    fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.md'), 'utf8').replace(/\r\n/g, '\n'));
  assert(summary.rows.filter((row) => row.baselineType === 'published-package').every((row) =>
    row.runtime === 'edge-isolate' && row.browserBundleBytes === null &&
    row.edgeBundleBytes === positive.edgeResults.deploymentRawBytes && row.metadataStripped === null));

  for (const result of [positive.edgeResults.probe, ...positive.edgeResults.results]) {
    const output = result.output;
    const bytes = fs.readFileSync(path.join(snapshot, path.basename(output.artifact)));
    const metadata = await sharp(bytes).metadata();
    await sharp(bytes).raw().toBuffer();
    assert.equal(hash(bytes), output.sha256);
    assert.equal(bytes.length, output.bytes);
    assert.equal(metadata.format, output.format);
    assert.equal(metadata.width, output.width);
    assert.equal(metadata.height, output.height);
  }
  const metadataFixture = positive.fixtures.find((item) => item.id === 'metadata-budget');
  const input = fs.readFileSync(path.join(snapshot, 'wasm-metadata-input.jpg'));
  const inputMetadata = await sharp(input).metadata();
  assert.equal(hash(input), metadataFixture.inputSha256);
  assert.deepEqual({ exif: Boolean(inputMetadata.exif),
    gpsTag: Boolean(inputMetadata.exif?.includes(Buffer.from([0x25, 0x88]))),
    xmp: Boolean(inputMetadata.xmp), icc: Boolean(inputMetadata.icc) },
  { exif: true, gpsTag: true, xmp: true, icc: true });
  const stripped = fs.readFileSync(path.join(snapshot, 'wasm-edge-metadata-budget.jpg'));
  const strippedMetadata = await sharp(stripped).metadata();
  assert.deepEqual({ exif: Boolean(strippedMetadata.exif), xmp: Boolean(strippedMetadata.xmp),
    icc: Boolean(strippedMetadata.icc) }, { exif: false, xmp: false, icc: false });
  const budget = positive.edgeResults.results.find((item) => item.id === 'metadata-budget').budget;
  const bestEffort = fs.readFileSync(path.join(snapshot, path.basename(budget.bestEffort.output.artifact)));
  assert.equal(hash(bestEffort), budget.bestEffort.output.sha256);
  assert(bestEffort.length > 10);
  assert.equal(budget.bestEffort.budgetMet, false);
  assert.equal(budget.strict.error.code, 'E502');

  const isolateIds = new Set([positive.edgeResults.probe, ...positive.edgeResults.results]
    .map((item) => item.isolateId));
  for (const [name, code, phase] of [
    ['corrupt-jpeg', 'E131', 'image-processing'],
    ['decoder-init', 'E131', 'optimizer-initialization'],
    ['resize-init', 'E503', 'optimizer-initialization'],
    ['encoder-init', 'E300', 'optimizer-initialization'],
  ]) {
    const evidence = read(name);
    const diagnostic = evidence.edgeResults.diagnostic;
    assert.equal(evidence.sourceSha, positive.sourceSha);
    assert.equal(evidence.sourceDirty, false);
    assert.equal(evidence.verdict, 'FAIL');
    assert.equal(evidence.diagnosticValidation.status, 'PASS');
    assert.equal(diagnostic.imageProcessingOutcome, 'FAIL');
    assert.equal(diagnostic.apiReached, true);
    assert.equal(diagnostic.failureStage, phase);
    assert.equal(diagnostic.apiError.code, code);
    assert.equal(diagnostic.apiError.category, 'CodecError');
    assert.equal(diagnostic.apiError.recoverable, false);
    assert.match(diagnostic.apiError.recoveryHint, /DevTools Network/);
    assert.match(diagnostic.apiError.message, /: .+/);
    assert.equal(diagnostic.apiError.isolateId, diagnostic.isolateId);
    assert(!isolateIds.has(diagnostic.isolateId), `${name} reused an isolate`);
    isolateIds.add(diagnostic.isolateId);
  }
  const failClosed = fs.readFileSync(path.join(snapshot, 'fail-closed.log'), 'utf8').trim()
    .split('\n').map((line) => JSON.parse(line));
  const missing = failClosed.find((entry) => entry.name === 'wasm-module-missing');
  assert(missing, 'missing static Wasm startup failure was not saved');
  assert.equal(missing.verdict, 'FAIL');
  assert.equal(missing.failureStage, 'isolate-launch');
  assert.equal(missing.apiReached, false);
  const mismatched = failClosed.find((entry) => entry.name === 'decoder-module-mismatch');
  assert.equal(mismatched.apiReached, true);
  assert.equal(mismatched.failedCases[0].apiError.code, 'E131');
  assert.equal(mismatched.failedCases[0].apiError.category, 'CodecError');
  assert.equal(mismatched.failedCases[0].apiError.recoverable, false);
  assert.match(mismatched.failedCases[0].apiError.recoveryHint, /DevTools Network/);
  assert.equal(mismatched.failedCases[0].wrapperHttpStatus, 500);
  console.log('published 1.4.0 Edge outputs, API diagnostics, and startup failure evidence PASS');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
