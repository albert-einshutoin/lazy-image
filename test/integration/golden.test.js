/**
 * ゴールデンテストスイート
 * - ピクセルハッシュでビット単位の回帰を検知
 * - sharpリファレンスとのSSIM/PSNRで画質回帰を数値監視
 */

const fs = require('fs');
const assert = require('assert');
const { resolveRoot } = require('../helpers/paths');
const cases = require('../golden/cases');
const expected = require('../golden/expected.json');
const { ImageEngine } = require(resolveRoot('index'));
const { hashPixels, applyOperationsToEngine, renderSharpReference } = require('../helpers/golden');
const { calculateQualityMetrics } = require('../helpers/quality');

let failed = 0;

async function runCase(testCase) {
    const baseline = expected[testCase.name];
    assert(baseline, `expected.json にケース ${testCase.name} の基準がありません`);

    const input = fs.readFileSync(testCase.input);

    // lazy-image 出力
    const engine = applyOperationsToEngine(ImageEngine.from(input), testCase.operations);
    const output = await engine.toBuffer(
        testCase.output.format,
        testCase.output.quality ?? undefined,
    );

    const { hash, info } = await hashPixels(output);
    assert.strictEqual(
        hash,
        baseline.pixelHash,
        `${testCase.name}: ピクセルハッシュ不一致 (expected ${baseline.pixelHash}, got ${hash})`,
    );
    assert.strictEqual(info.width, baseline.width, `${testCase.name}: width mismatch`);
    assert.strictEqual(info.height, baseline.height, `${testCase.name}: height mismatch`);

    // sharpリファレンスとの画質比較
    const reference = await renderSharpReference(input, testCase.operations, testCase.output);
    const { psnr, ssim } = await calculateQualityMetrics(reference, output);

    assert(
        ssim >= testCase.thresholds.minSsim,
        `${testCase.name}: SSIM ${ssim.toFixed(4)} < ${testCase.thresholds.minSsim}`,
    );
    assert(
        psnr >= testCase.thresholds.minPsnr,
        `${testCase.name}: PSNR ${psnr.toFixed(2)} < ${testCase.thresholds.minPsnr}`,
    );

    console.log(
        `✅ ${testCase.name} | hash=${hash.slice(0, 8)}… | ${info.width}x${info.height} | SSIM=${ssim.toFixed(4)} PSNR=${psnr.toFixed(2)}`,
    );
}

async function run() {
    console.log('=== Golden Test Suite ===');
    for (const testCase of cases) {
        try {
            await runCase(testCase);
        } catch (err) {
            failed += 1;
            console.error(`❌ ${testCase.name}`);
            console.error(`   ${err.message}`);
        }
    }

    if (failed > 0) {
        console.error(`\n❌ ${failed} golden case(s) failed`);
        process.exit(1);
    }
    console.log('\n✅ All golden cases passed');
}

run();
