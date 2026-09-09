/**
 * Regression coverage for benchmark corpus manifests.
 *
 * Backend bake-off artifacts should identify the exact input corpus and the
 * resized reference PNGs used for codec comparison.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const {
  buildCorpusManifestMarkdown,
  describeReferenceCorpusEntry,
  readCorpusManifest,
  verifyCorpusManifest,
} = require('../helpers/benchmark-corpus');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');

let passed = 0;
let failed = 0;

function report(name, error) {
  if (error) {
    console.log(`not ok - ${name}`);
    console.log(`   Error: ${error.message}`);
    failed += 1;
  } else {
    console.log(`ok - ${name}`);
    passed += 1;
  }
}

async function test(name, fn) {
  try {
    await fn();
    report(name);
  } catch (error) {
    report(name, error);
  }
}

console.log('=== Benchmark Corpus Manifest Tests ===\n');

const outputDir = resolveTemp('integration', 'benchmark-corpus-manifest');
fs.mkdirSync(outputDir, { recursive: true });

async function main() {
  await test('verifies the tracked compile-image corpus license and checksums', async () => {
    const manifestPath = resolveRoot('test/benchmarks/corpus/manifest.json');
    const verified = verifyCorpusManifest(readCorpusManifest(manifestPath), resolveRoot());

    assert.equal(verified.name, 'compile-image-minimal-v1');
    assert.equal(verified.license.spdx, 'MIT');
    assert.equal(verified.entries.length, 3);
    assert.deepEqual(
      verified.entries.map((entry) => entry.id),
      ['jpeg-verification-io', 'orientation-absent-exif', 'transparent-avif'],
    );
    assert.equal(verified.entries.find((entry) => entry.id === 'transparent-avif').expectations.outputAlpha, true);
  });

  await test('rejects corpus checksum and path drift', async () => {
    const manifest = readCorpusManifest(resolveRoot('test/benchmarks/corpus/manifest.json'));
    const checksumDrift = JSON.parse(JSON.stringify(manifest));
    checksumDrift.entries[0].sha256 = '0'.repeat(64);
    assert.throws(() => verifyCorpusManifest(checksumDrift, resolveRoot()), /checksum mismatch/);

    const licenseDrift = JSON.parse(JSON.stringify(manifest));
    licenseDrift.license.sha256 = '0'.repeat(64);
    assert.throws(() => verifyCorpusManifest(licenseDrift, resolveRoot()), /license checksum/);

    const pathDrift = JSON.parse(JSON.stringify(manifest));
    pathDrift.entries[0].path = '../LICENSE';
    assert.throws(() => verifyCorpusManifest(pathDrift, resolveRoot()), /stay within the corpus root/);
  });

  await test('records fixture source and resized reference PNG digests', async () => {
    const sourcePath = resolveFixture('test_38kb_input.jpg');
    const referencePng = await sharp(sourcePath).resize(64, null, { withoutEnlargement: true }).png().toBuffer();
    const referencePngPath = path.join(outputDir, 'fixture-reference.png');
    fs.writeFileSync(referencePngPath, referencePng);

    const entry = await describeReferenceCorpusEntry({
      label: 'fixture-case',
      sourcePath,
      referencePng,
      referencePngPath,
      settings: {
        width: 64,
        quality: 80,
      },
    });

    assert.equal(entry.label, 'fixture-case');
    assert.equal(entry.source.kind, 'fixture');
    assert.equal(entry.source.bytes, fs.statSync(sourcePath).size);
    assert.match(entry.source.sha256, /^[a-f0-9]{64}$/);
    assert.match(entry.referencePng.sha256, /^[a-f0-9]{64}$/);
    assert.equal(entry.referencePng.width, 64);
    assert.equal(entry.referencePng.bytes, referencePng.length);
    assert.equal(entry.settings.quality, 80);
  });

  await test('records generated corpus entries without pretending they are fixtures', async () => {
    const referencePng = await sharp({
      create: {
        width: 12,
        height: 8,
        channels: 4,
        background: { r: 255, g: 0, b: 0, alpha: 0.5 },
      },
    })
      .png()
      .toBuffer();
    const referencePngPath = path.join(outputDir, 'generated-reference.png');
    fs.writeFileSync(referencePngPath, referencePng);

    const entry = await describeReferenceCorpusEntry({
      label: 'generated-case',
      sourceKind: 'generated-alpha-gradient',
      generatedBy: 'generated-alpha-gradient',
      referencePng,
      referencePngPath,
      expectations: {
        alpha: true,
        icc: false,
      },
    });

    assert.equal(entry.source.kind, 'generated-alpha-gradient');
    assert.equal(entry.source.path, null);
    assert.equal(entry.source.sha256, null);
    assert.equal(entry.referencePng.hasAlpha, true);
    assert.equal(entry.expectations.alpha, true);
  });

  await test('renders corpus manifest markdown with compact digests and settings', async () => {
    const sourcePath = resolveFixture('test_38kb_input.jpg');
    const referencePng = await sharp(sourcePath).resize(32, null, { withoutEnlargement: true }).png().toBuffer();
    const referencePngPath = path.join(outputDir, 'markdown-reference.png');
    fs.writeFileSync(referencePngPath, referencePng);

    const entry = await describeReferenceCorpusEntry({
      label: 'markdown-case',
      sourcePath,
      referencePng,
      referencePngPath,
      settings: {
        width: 32,
        mozjpegQuality: 80,
      },
    });
    const markdown = buildCorpusManifestMarkdown([entry]).join('\n');

    assert.match(markdown, /## Corpus Manifest/);
    assert.match(markdown, /markdown-case/);
    assert.match(markdown, /test\/fixtures\/test_38kb_input\.jpg/);
    assert.doesNotMatch(markdown, /test\\fixtures\\test_38kb_input\.jpg/);
    assert.match(markdown, /width=32, mozjpegQuality=80/);
    assert.match(markdown, /[a-f0-9]{12}/);
  });

  console.log(`\nTests passed: ${passed}, failed: ${failed}`);
  if (failed > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
