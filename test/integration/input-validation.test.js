/**
 * Input validation coverage for API boundaries.
 */

const fs = require('fs');
const assert = require('assert');
const { resolveRoot, resolveFixture, resolveTemp } = require('../helpers/paths');
const { ImageEngine, ErrorCategory, getErrorCategory } = require(resolveRoot('index'));

const TEST_IMAGE = resolveFixture('test_input.jpg');
const BUFFER = fs.readFileSync(TEST_IMAGE);

let passed = 0;
let failed = 0;

function report(name, error) {
    if (error) {
        console.log(`❌ ${name}`);
        console.log(`   Error: ${error.message}`);
        failed++;
    } else {
        console.log(`✅ ${name}`);
        passed++;
    }
}

async function asyncTest(name, fn) {
    try {
        await fn();
        report(name);
    } catch (e) {
        report(name, e);
    }
}

(async () => {
    await asyncTest('resize rejects negative width with structured error', async () => {
        let threw = false;
        try {
            ImageEngine.from(BUFFER).resize(-10);
        } catch (e) {
            threw = true;
            assert.strictEqual(e.errorCode, 'E203', 'errorCode should be E203 for resize validation');
            const category = getErrorCategory(e);
            assert.strictEqual(category, ErrorCategory.UserError, 'category should be UserError');
            assert(e.recoveryHint && e.recoveryHint.length > 0, 'recoveryHint should be present');
        }
        assert(threw, 'negative resize width should throw synchronously');
    });

    await asyncTest('toBuffer rejects NaN quality', async () => {
        let threw = false;
        try {
            await ImageEngine.from(BUFFER).resize(100).toBuffer('jpeg', Number.NaN);
        } catch (e) {
            threw = true;
            assert.strictEqual(e.errorCode, 'E400', 'invalid_argument should map to E400');
            const category = getErrorCategory(e);
            assert.strictEqual(category, ErrorCategory.UserError, 'category should be UserError');
            assert(e.message.toLowerCase().includes('quality'), 'message should mention quality');
        }
        assert(threw, 'NaN quality should throw');
    });

    await asyncTest('processBatch rejects negative concurrency', async () => {
        const tmpDir = resolveTemp('validation_negative_concurrency');
        let threw = false;
        try {
            await ImageEngine.from(BUFFER).resize(50).processBatch(
                [TEST_IMAGE],
                tmpDir,
                {
                    format: 'jpeg',
                    quality: 80,
                    concurrency: -1,
                }
            );
        } catch (e) {
            threw = true;
            assert.strictEqual(e.errorCode, 'E400', 'invalid_argument should map to E400');
            const category = getErrorCategory(e);
            assert.strictEqual(category, ErrorCategory.UserError, 'category should be UserError');
            assert(e.message.toLowerCase().includes('concurrency'), 'message should mention concurrency');
        }
        assert(threw, 'negative concurrency should throw');
    });

    await asyncTest('toFile rejects empty path eagerly', async () => {
        let threw = false;
        try {
            await ImageEngine.from(BUFFER).resize(10).toFile('', 'jpeg', 80);
        } catch (e) {
            threw = true;
            assert.strictEqual(e.errorCode, 'E400', 'invalid_argument should map to E400');
            const category = getErrorCategory(e);
            assert.strictEqual(category, ErrorCategory.UserError, 'category should be UserError');
            assert(e.message.toLowerCase().includes('path'), 'message should mention path');
        }
        assert(threw, 'empty output path should throw before scheduling task');
    });

    console.log(`\nTests passed: ${passed}, failed: ${failed}`);
    if (failed > 0) {
        process.exit(1);
    }
})();

