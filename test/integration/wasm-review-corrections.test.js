const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createHash } = require('node:crypto');
const path = require('node:path');
const sharp = require('sharp');
const { publishedRow, metadataVerification, renderMarkdownReport, SCENARIOS } = require('../benchmarks/wasm-upload-comparison.bench');

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
  const edgeMarkdown = renderMarkdownReport({ ...report, edgeMeasured: true,
    metadataVerification: edgeMetadata, rows: edgeRows });
  for (const row of edgeRows) {
    const line = edgeMarkdown.split('\n').find((item) => item.startsWith(`| ${row.scenario} | ${row.runtime} |`));
    const cells = line.split('|').slice(1, -1).map((item) => item.trim());
    assert.equal(cells.length, 25);
    assert.equal(cells[5] === 'n/a', row.runtime !== 'browser-worker');
    assert.equal(cells[7] === 'n/a', row.runtime !== 'edge-isolate');
  }
  console.log('Wasm review corrections: saved input, ICC negative, runtime rows, JSON/Markdown PASS');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
