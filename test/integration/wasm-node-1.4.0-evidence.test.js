const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');

const snapshot = path.resolve(__dirname, '../../docs/history/v1.4.0/node-wasm');
const read = (name) => JSON.parse(fs.readFileSync(path.join(snapshot, `${name}-evidence.json`)));
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');

async function main() {
  const positive = read('positive');
  const corrupt = read('corrupt-jpeg');
  const resize = read('resize-init');
  const runs = [positive, corrupt, resize];
  assert.equal(positive.sourceSha, '4c356dcd7fc90a7917dfa378f2480e07ccab0356');
  assert.equal(positive.sourceFilesSha256,
    'f6c87568e850100ed181e68e1bab8a299825495e5661227cdaed097b39313404');
  assert.equal(new Set(runs.map((run) => run.nodeExecution.processId)).size, 3);
  for (const run of runs) {
    assert.equal(run.sourceSha, positive.sourceSha);
    assert.equal(run.sourceDirty, false);
    assert.equal(run.publishedVersion, '1.4.0');
    assert.equal(run.packageSource.kind, 'published-registry');
    assert.equal(run.VERSION, '1.4.0');
    assert.equal(run.browserExport, './browser');
    assert(run.importPath.endsWith('/node_modules/@alberteinshutoin/lazy-image-wasm/browser.js'));
    assert.equal(run.packages['@alberteinshutoin/lazy-image-wasm'].integrity,
      positive.packages['@alberteinshutoin/lazy-image-wasm'].integrity);
    assert.equal(run.nodeExecution.label, 'Node上のWasm実行／ImageData shim・Wasm bytes明示注入');
    assert.equal(run.nodeModuleAssignments.length, 5);
  }
  assert.equal(positive.verdict, 'PASS');
  assert.equal(positive.packages['@alberteinshutoin/lazy-image-wasm'].integrity,
    'sha512-R3xpMJr3rBv+LsyfpggixCZvktlS3XRvLi542x4JgbrvvBP3NH/xip3TJSze4fGDQIfj7hWplLeKUlHPCnidew==');
  assert.equal(positive.nodeStage, 'completed');
  assert.deepEqual(positive.nodeTotals, { imageConversionsPassed: 4, imageConversionsAttempted: 4,
    budgetMet: 3, budgetAttempts: 4, expectedStrictRejections: 1, strictAttempts: 1 });
  const summary = JSON.parse(fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.json')));
  const { renderMarkdownReport } = require('../benchmarks/wasm-upload-comparison.bench');
  assert.equal(renderMarkdownReport(summary),
    fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.md'), 'utf8'));
  assert.equal(summary.metadataVerification.rawEvidence, summary.artifactPaths.publishedEvidence);
  const rows = summary.rows.filter((row) => row.baselineType === 'published-package');
  assert.equal(rows.length, 2);
  assert(rows.every((row) => row.runtime === 'node-wasm' && row.browserBundleBytes === null &&
    row.browserBundleGzipBytes === null && row.edgeBundleBytes === null &&
    row.edgeBundleGzipBytes === null && row.metadataStripped === null));

  for (const result of positive.nodeResults) {
    const output = result.output;
    const bytes = fs.readFileSync(path.join(snapshot, path.basename(output.artifact)));
    const metadata = await sharp(bytes).metadata();
    await sharp(bytes).raw().toBuffer();
    assert.equal(hash(bytes), output.sha256);
    assert.equal(bytes.length, output.bytes);
    assert.equal(metadata.format, output.format);
    assert.equal(metadata.width, output.width);
    assert.equal(metadata.height, output.height);
    assert.equal(result.metrics.bytesOut, bytes.length);
    assert.equal(result.metrics.budgetMet, true);
  }
  const metadataFixture = positive.fixtures.find((item) => item.id === 'metadata-budget');
  const input = fs.readFileSync(path.join(snapshot, 'wasm-metadata-input.jpg'));
  const inputMetadata = await sharp(input).metadata();
  assert.equal(hash(input), metadataFixture.inputSha256);
  assert.deepEqual({ exif: Boolean(inputMetadata.exif),
    gpsTag: Boolean(inputMetadata.exif?.includes(Buffer.from([0x25, 0x88]))),
    xmp: Boolean(inputMetadata.xmp), icc: Boolean(inputMetadata.icc) },
  { exif: true, gpsTag: true, xmp: true, icc: true });
  const outputMetadata = positive.nodeResults.find((entry) => entry.id === 'metadata-budget').output.metadata;
  assert.deepEqual(outputMetadata, { exif: false, xmp: false, icc: false });
  const budget = positive.nodeResults.find((entry) => entry.id === 'metadata-budget').budget;
  const bestEffort = fs.readFileSync(path.join(snapshot, path.basename(budget.bestEffort.output.artifact)));
  await sharp(bestEffort).raw().toBuffer();
  assert.equal(hash(bestEffort), budget.bestEffort.output.sha256);
  assert.equal(bestEffort.length, budget.bestEffort.bytesOut);
  assert(bestEffort.length > 10);
  assert.equal(budget.bestEffort.budgetMet, false);
  assert.equal(budget.strict.code, 'E502');
  assert.equal(budget.strict.expectedRejection, true);

  for (const [run, code, stage, inputFile] of [
    [corrupt, 'E131', 'image-processing', 'wasm-node-corrupt-jpeg-input.jpg'],
    [resize, 'E503', 'optimizer-initialization', 'wasm-node-resize-init-input.jpg'],
  ]) {
    assert.equal(run.verdict, 'FAIL');
    assert.equal(run.diagnosticValidation.status, 'PASS');
    assert.equal(run.nodeStage, stage);
    assert.equal(run.nodeDiagnostic.imageProcessingOutcome, 'FAIL');
    assert.equal(run.nodeDiagnostic.failureStage, stage);
    assert.equal(run.nodeDiagnostic.apiError.code, code);
    assert.equal(run.nodeDiagnostic.apiError.category, 'CodecError');
    assert.equal(run.nodeDiagnostic.apiError.recoverable, false);
    assert.match(run.nodeDiagnostic.apiError.recoveryHint, /DevTools Network/);
    const bytes = fs.readFileSync(path.join(snapshot, inputFile));
    assert.equal(hash(bytes), run.nodeDiagnostic.input.sha256);
  }
  assert.deepEqual(fs.readFileSync(path.join(snapshot, 'wasm-node-corrupt-jpeg-input.jpg')),
    Buffer.from([0xff, 0xd8, 0xff, 0, 0, 0]));
  assert.equal((await sharp(fs.readFileSync(path.join(snapshot, 'wasm-node-resize-init-input.jpg')))
    .metadata()).format, 'jpeg');
  const publicResize = positive.nodeModuleAssignments.find((entry) => entry.key === 'resize');
  const invalidResize = resize.nodeModuleAssignments.find((entry) => entry.key === 'resize');
  assert.equal(invalidResize.source, 'explicit-invalid-bytes');
  assert.equal(invalidResize.originalSha256, publicResize.sha256);
  assert.equal(invalidResize.bytesHex, '00');
  console.log('published 1.4.0 Node outputs, budget, metadata, and API diagnostics PASS');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
