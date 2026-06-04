/**
 * Regression coverage for replaying generated malformed fuzz seeds through NAPI.
 *
 * The fuzz workflow builds sanitizer coverage around these seeds. Keep a short
 * integration replay too, so corpus growth is checked against the JS boundary.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resolveRoot, resolveTemp } = require('../helpers/paths');
const { ImageEngine, ErrorCategory, getErrorCategory } = require(resolveRoot('index'));

const SEED_SCRIPT = resolveRoot('scripts/fuzz/write-seed-corpus.mjs');
const CORPUS_ROOT = resolveTemp(`malformed-corpus-replay-${process.pid}`);

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

async function asyncTest(name, fn) {
  try {
    await fn();
    report(name);
  } catch (error) {
    report(name, error);
  }
}

function resetCorpusRoot() {
  fs.rmSync(CORPUS_ROOT, { recursive: true, force: true });
}

function generateCorpus() {
  const result = spawnSync(process.execPath, [
    SEED_SCRIPT,
    '--corpus-root',
    CORPUS_ROOT,
  ], {
    cwd: resolveRoot(),
    encoding: 'utf8',
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /wrote 20 malformed fuzz seed\(s\)/);
}

function collectCorpusFiles() {
  const files = [];

  for (const target of fs.readdirSync(CORPUS_ROOT).sort()) {
    const targetDir = path.join(CORPUS_ROOT, target);
    for (const name of fs.readdirSync(targetDir).sort()) {
      const seedPath = path.join(targetDir, name);
      files.push({
        target,
        label: `${target}/${name}`,
        path: seedPath,
        bytes: fs.statSync(seedPath).size,
      });
    }
  }

  return files;
}

function assertExpectedMalformedRejection(error, label) {
  assert(error, `${label} should reject`);
  assert.notEqual(error.errorCode, 'E900', `${label} should not surface an internal panic`);
  assert.notEqual(error.errorCode, 'E901', `${label} should not surface a generic internal error`);

  const category = getErrorCategory(error);
  assert.notEqual(category, null, `${label} should include a known error category`);
  assert.notEqual(category, ErrorCategory.InternalBug, `${label} should not be categorized as InternalBug`);
}

async function expectRejects(label, run) {
  let error = null;

  try {
    await run();
  } catch (err) {
    error = err;
  }

  assertExpectedMalformedRejection(error, label);
}

console.log('=== Malformed Corpus Replay Tests ===\n');

(async () => {
  resetCorpusRoot();

  try {
    generateCorpus();
    const files = collectCorpusFiles();

    await asyncTest('generated corpus keeps the expected target coverage', async () => {
      assert.equal(files.length, 20);
      assert.deepEqual(
        Array.from(new Set(files.map((file) => file.target))).sort(),
        ['decode_avif', 'decode_from_buffer', 'exif_parse', 'icc_profile', 'inspect_header'],
      );
      assert.ok(files.every((file) => file.bytes >= 0));
    });

    await asyncTest('generated malformed seeds reject through buffer input', async () => {
      for (const file of files) {
        const input = fs.readFileSync(file.path);
        await expectRejects(`buffer:${file.label}`, () => (
          ImageEngine.from(input)
            .sanitize({ policy: 'strict' })
            .toBuffer('jpeg', 80)
        ));
      }
    });

    await asyncTest('generated malformed seeds reject through file input', async () => {
      for (const file of files) {
        await expectRejects(`file:${file.label}`, () => (
          ImageEngine.fromPath(file.path)
            .sanitize({ policy: 'strict' })
            .toBuffer('jpeg', 80)
        ));
      }
    });
  } finally {
    resetCorpusRoot();
  }

  console.log(`\nTests passed: ${passed}, failed: ${failed}`);
  if (failed > 0) {
    process.exit(1);
  }
})();
