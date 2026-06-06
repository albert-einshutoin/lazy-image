/**
 * Regression coverage for the malformed fuzz seed generator.
 *
 * The fuzz workflow uses this script before each target run; keep its target
 * selection and generated corpus shape stable in local JS tests too.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { resolveRoot, resolveTemp } = require('../helpers/paths');

const SCRIPT_PATH = resolveRoot('scripts/fuzz/write-seed-corpus.mjs');
const TEST_ROOT = resolveTemp(`fuzz-seed-corpus-${process.pid}`);

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

function test(name, fn) {
    try {
        fn();
        report(name);
    } catch (error) {
        report(name, error);
    }
}

function runSeedScript(args) {
    const result = spawnSync(process.execPath, [SCRIPT_PATH, ...args], {
        cwd: resolveRoot(),
        encoding: 'utf8',
    });

    assert.equal(result.status, 0, result.stderr || result.stdout);
    return result.stdout;
}

function resetTestRoot() {
    fs.rmSync(TEST_ROOT, { recursive: true, force: true });
}

function fileSize(...segments) {
    return fs.statSync(path.join(TEST_ROOT, ...segments)).size;
}

console.log('=== Fuzz Seed Corpus Script Tests ===\n');

try {
    resetTestRoot();

    test('dry-run validates the full malformed seed catalog', () => {
        const stdout = runSeedScript(['--dry-run']);
        assert.match(stdout, /validated 20 malformed fuzz seed\(s\)/);
        assert.match(stdout, /avif-ftyp-only\.bin/);
        assert.match(stdout, /jpeg-truncated-exif-app1\.bin/);
        assert.match(stdout, /png-huge-ihdr\.bin/);
    });

    test('decode_avif target writes AVIF-like malformed seeds only', () => {
        const stdout = runSeedScript([
            '--target',
            'decode_avif',
            '--corpus-root',
            TEST_ROOT,
        ]);

        assert.match(stdout, /wrote 3 malformed fuzz seed\(s\)/);
        assert.equal(fileSize('decode_avif', 'avif-ftyp-only.bin'), 24);
        assert.equal(fileSize('decode_avif', 'avif-truncated-meta-box.bin'), 36);
        assert.equal(fileSize('decode_avif', 'avif-invalid-box-size.bin'), 12);
    });

    test('decode_from_buffer target includes generic malformed image headers', () => {
        const stdout = runSeedScript([
            '--target',
            'decode_from_buffer',
            '--corpus-root',
            TEST_ROOT,
        ]);

        assert.match(stdout, /wrote 6 malformed fuzz seed\(s\)/);
        assert.equal(fileSize('decode_from_buffer', 'empty.bin'), 0);
        assert.equal(fileSize('decode_from_buffer', 'png-huge-ihdr.bin'), 33);
        assert.equal(fileSize('decode_from_buffer', 'webp-truncated-riff.bin'), 16);
    });

    test('structured-input targets are intentionally left without raw seed corpus', () => {
        const stdout = runSeedScript([
            '--target',
            'firewall_bypass',
            '--corpus-root',
            TEST_ROOT,
        ]);

        assert.match(stdout, /no malformed seed corpus configured for target: firewall_bypass/);
        assert.equal(fs.existsSync(path.join(TEST_ROOT, 'firewall_bypass')), false);
    });
} finally {
    resetTestRoot();
}

console.log(`\nTests passed: ${passed}, failed: ${failed}`);
if (failed > 0) {
    process.exit(1);
}
