const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createHash } = require('node:crypto');
const path = require('node:path');
const sharp = require('sharp');
const { publishedRow, metadataVerification, renderMarkdownReport, reaggregate, SCENARIOS } = require('../benchmarks/wasm-upload-comparison.bench');

async function main() {
  const root = path.resolve(__dirname, '../..');
  const snapshot = path.join(root, 'docs/history/wasm-1.3.1');
  const evidence = JSON.parse(fs.readFileSync(path.join(snapshot, 'wasm-published-evidence.json')));
  const savedInput = fs.readFileSync(path.join(snapshot, 'wasm-metadata-input.jpg'));
  const input = evidence.fixtures.find((item) => item.id === 'metadata-budget');
  assert.equal(createHash('sha256').update(savedInput).digest('hex'), input.inputSha256);
  const { assertMetadataInput } = await import('../benchmarks/wasm-published-evidence.mjs');
  const inspect = async (bytes) => {
    const meta = await sharp(bytes).metadata();
    return { exif: Boolean(meta.exif), gpsTag: Boolean(meta.exif?.includes(Buffer.from([0x25, 0x88]))),
      xmp: Boolean(meta.xmp), icc: Boolean(meta.icc) };
  };
  const actualInput = await inspect(savedInput);
  assert.deepEqual(actualInput, input.inputMetadata);
  assertMetadataInput(actualInput);

  const noIcc = await sharp(savedInput).keepExif().withXmp(
    '<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about=""/></rdf:RDF></x:xmpmeta>'
  ).jpeg().toBuffer();
  const actualNoIcc = await inspect(noIcc);
  assert.deepEqual(actualNoIcc, { exif: true, gpsTag: true, xmp: true, icc: false });
  assert.throws(() => assertMetadataInput(actualNoIcc), /missing required input: icc/);

  const allRows = SCENARIOS.flatMap((scenario) => [
    publishedRow(scenario, 'node-wasm', evidence), publishedRow(scenario, 'browser-worker', evidence),
  ]);
  const nodeOnlyRows = SCENARIOS.map((scenario) => publishedRow(scenario, 'node-wasm', evidence));
  for (const row of [...nodeOnlyRows, ...allRows.filter((item) => item.runtime === 'node-wasm')]) {
    assert.equal(row.browserBundleBytes, null);
    assert.equal(row.browserBundleGzipBytes, null);
    assert.equal(row.metadataStripped, null);
  }
  for (const row of allRows.filter((item) => item.runtime === 'browser-worker')) {
    assert.equal(row.browserBundleBytes, 1245431);
    assert.equal(row.browserBundleGzipBytes, 407545);
    assert.equal(row.metadataStripped, null);
  }
  const metadata = metadataVerification(evidence, ['node-wasm', 'browser-worker']);
  for (const result of Object.values(metadata.results)) {
    assert.deepEqual(result.removed, { exif: true, gpsTag: true, xmp: true, icc: true });
    assert.deepEqual(result.outputMetadata, { exif: false, xmp: false, icc: false });
  }
  const report = { generatedAt: evidence.generatedAt, sourceSha: evidence.sourceSha,
    publishedVersion: evidence.publishedVersion, command: evidence.command,
    artifactPaths: { publishedEvidence: metadata.rawEvidence }, metadataVerification: metadata,
    rows: allRows };
  const json = JSON.parse(JSON.stringify(report));
  const markdown = renderMarkdownReport(json);
  for (const row of json.rows) {
    const line = markdown.split('\n').find((item) => item.startsWith(`| ${row.scenario} | ${row.runtime} |`));
    assert(line, `missing markdown row: ${row.runtime}/${row.scenario}`);
    const cells = line.split('|').slice(1, -1).map((item) => item.trim());
    assert.equal(cells[18], 'n/a');
    assert.equal(cells[5], row.runtime === 'browser-worker' ? '1.2 MB' : 'n/a');
    assert.equal(cells[6], row.runtime === 'browser-worker' ? '398.0 KB' : 'n/a');
  }
  assert(markdown.includes('| node-wasm | true | true | true | true | false | false | false | true |'));
  assert(markdown.includes('| browser-worker | true | true | true | true | false | false | false | true |'));
  const savedSummary = JSON.parse(fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.json')));
  assert(savedSummary.rows.filter((row) => row.baselineType === 'native-reference' ||
    row.baselineType === 'published-package').every((row) => row.metadataStripped === null));
  assert.deepEqual(savedSummary.metadataVerification, metadata);
  assert.equal(renderMarkdownReport(savedSummary), fs.readFileSync(path.join(snapshot, 'wasm-upload-summary.md'), 'utf8'));

  const edgeEvidence = { ...evidence, edgeResults: { deploymentRawBytes: 777000,
    deploymentGzipBytes: 222000, results: evidence.nodeResults.map((entry) =>
      ({ ...entry, coldFromBeforeRuntimeMs: 100, firstRequestMs: 90, startupToReadyMs: 10 })) } };
  const edgeRows = SCENARIOS.flatMap((scenario) => ['node-wasm', 'browser-worker', 'edge-isolate']
    .map((runtime) => publishedRow(scenario, runtime, edgeEvidence)));
  for (const row of edgeRows) {
    assert.equal(row.browserBundleBytes === null, row.runtime !== 'browser-worker');
    assert.equal(row.edgeBundleBytes === null, row.runtime !== 'edge-isolate');
    if (row.runtime === 'edge-isolate') {
      assert.equal(row.edgeBundleBytes, 777000);
      assert.equal(row.edgeBundleGzipBytes, 222000);
    }
  }
  const edgeMetadata = metadataVerification(edgeEvidence, ['node-wasm', 'browser-worker', 'edge-isolate']);
  assert(edgeMetadata.results['edge-isolate']);
  const tmp = fs.mkdtempSync(path.join(require('node:os').tmpdir(), 'lazy-image-reaggregate-'));
  try {
    const raw = path.join(tmp, 'raw.json');
    const previous = path.join(tmp, 'previous.json');
    fs.writeFileSync(raw, JSON.stringify(edgeEvidence));
    fs.writeFileSync(previous, JSON.stringify({ ...savedSummary, rows: edgeRows, edgeMeasured: true }));
    reaggregate(raw, previous);
    const corrected = JSON.parse(fs.readFileSync(path.join(root, 'artifacts/benchmark/wasm-upload-summary.json')));
    assert.equal(corrected.rows.filter((row) => row.baselineType === 'published-package').length, 6);
    assert.equal(corrected.edgeMeasured, true);
    assert.equal(corrected.metadataVerification.rawEvidence, corrected.artifactPaths.publishedEvidence);
    assert.equal(renderMarkdownReport(corrected),
      fs.readFileSync(path.join(root, 'artifacts/benchmark/wasm-upload-summary.md'), 'utf8'));
    fs.writeFileSync(previous, JSON.stringify({ ...savedSummary, rows: edgeRows.slice(1), edgeMeasured: true }));
    assert.throws(() => reaggregate(raw, previous), /exactly one published row per scenario\/runtime/);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
  const edgeMarkdown = renderMarkdownReport({ ...report, edgeMeasured: true,
    metadataVerification: edgeMetadata, rows: edgeRows });
  assert.match(edgeMarkdown, /Browser\/Node\/Edge details and output hashes/);
  const edgeOnlyMarkdown = renderMarkdownReport({ ...report, edgeMeasured: true,
    metadataVerification: edgeMetadata, rows: edgeRows.filter((row) => row.runtime === 'edge-isolate') });
  assert.match(edgeOnlyMarkdown, /Edge details and output hashes/);
  assert.doesNotMatch(edgeOnlyMarkdown, /(?:Browser|Node)\/.*Edge details/);
  for (const row of edgeRows) {
    const line = edgeMarkdown.split('\n').find((item) => item.startsWith(`| ${row.scenario} | ${row.runtime} |`));
    const cells = line.split('|').slice(1, -1).map((item) => item.trim());
    assert.equal(cells.length, 25);
    assert.equal(cells[5] === 'n/a', row.runtime !== 'browser-worker');
    assert.equal(cells[7] === 'n/a', row.runtime !== 'edge-isolate');
  }
  const savedEdge = JSON.parse(fs.readFileSync(path.join(snapshot, 'edge-workerd/wasm-published-evidence.json')));
  assert.equal(savedEdge.packages.esbuild.licenseFiles['LICENSE.md'],
    'b40ec5baec7bb34fa5b1c09521fa3cd52d5fad7adafed74932a2010d3612a681');
  assert.equal(savedEdge.packages.workerd.licenseSource.sha256,
    '0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594');
  assert.equal(savedEdge.toolchainBinaries.workerd.sha256,
    '354615e8d5ccbc2ab9afff5ec7f9a3a6e21f83bb27c314c4c65685d1fbaf984c');
  assert(savedEdge.packages['@cloudflare/workerd-darwin-arm64']?.integrity);
  const savedEdgeSummary = JSON.parse(fs.readFileSync(path.join(snapshot, 'edge-workerd/wasm-upload-summary.json')));
  assert.equal(renderMarkdownReport(savedEdgeSummary),
    fs.readFileSync(path.join(snapshot, 'edge-workerd/wasm-upload-summary.md'), 'utf8'));
  const defaultDir = path.join(snapshot, 'browser-default');
  const defaultEvidence = JSON.parse(fs.readFileSync(path.join(defaultDir, 'wasm-published-evidence.json')));
  const defaultBrowser = defaultEvidence.browserResults;
  assert.equal(defaultEvidence.verdict, 'PASS');
  assert.equal(defaultEvidence.sourceDirty, false);
  assert.equal(defaultEvidence.publishedVersion, '1.3.1');
  assert.equal(defaultEvidence.browserLoadMode, 'default');
  assert(defaultBrowser.setup.every((entry) => Object.keys(entry.assetBytes).length === 0));
  assert(defaultBrowser.requestedWasm.some((asset) => asset.url === '/webp_enc_simd.wasm'));
  assert.deepEqual(defaultBrowser.requiredAssets,
    [...new Set(defaultBrowser.requests.filter((request) => request.status === 200 &&
      /\.(?:js|wasm)$/.test(request.url)).map((request) => request.url))]);
  for (const result of [defaultBrowser.probe, ...defaultBrowser.results]) {
    const bytes = fs.readFileSync(path.join(defaultDir, path.basename(result.output.artifact)));
    assert.equal(createHash('sha256').update(bytes).digest('hex'), result.output.sha256);
    assert.equal(bytes.length, result.output.bytes);
  }
  assert.deepEqual(defaultBrowser.totals, { imageConversionsPassed: 4, imageConversionsAttempted: 4,
    budgetMet: 3, budgetAttempts: 4, expectedStrictRejections: 1, strictAttempts: 1 });
  const defaultSummary = JSON.parse(fs.readFileSync(path.join(defaultDir, 'wasm-upload-summary.json')));
  assert.equal(defaultSummary.browserLoadMode, 'default');
  assert(defaultSummary.rows.filter((row) => row.baselineType === 'published-package')
    .every((row) => row.runtime === 'browser-worker' && row.loadMode.includes('default loader') &&
      row.browserBundleBytes === defaultBrowser.deploymentRawBytes && row.metadataStripped === null));
  assert.equal(renderMarkdownReport(defaultSummary),
    fs.readFileSync(path.join(defaultDir, 'wasm-upload-summary.md'), 'utf8'));
  const missingWasm = JSON.parse(fs.readFileSync(path.join(defaultDir, 'missing-wasm-evidence.json')));
  assert.equal(missingWasm.verdict, 'FAIL');
  assert(missingWasm.browserResults.requests.some((request) =>
    request.url === '/mozjpeg_dec.wasm' && request.status === 404));
  assert.equal(missingWasm.browserResults.cases[0].cold.error.code, 'E131');
  console.log('Wasm review corrections: saved input, ICC negative, runtime rows, JSON/Markdown PASS');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
