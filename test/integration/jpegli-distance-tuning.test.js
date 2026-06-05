/**
 * Regression coverage for jpegli distance tuning decisions.
 *
 * The bake-off harness can be smoke-tested without cjpegli, but the
 * matched-byte and matched-quality candidate selection rules must remain
 * deterministic in normal CI.
 */

const assert = require('node:assert/strict');
const {
  buildDistanceCandidates,
  buildTuningCandidate,
  chooseMatchedBytesCandidate,
  chooseMatchedQualityCandidate,
  parseDistanceCandidateList,
} = require('../helpers/jpegli-distance-tuning');

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

console.log('=== jpegli Distance Tuning Tests ===\n');

test('parses and normalizes distance candidates with the case baseline included', () => {
  const parsed = parseDistanceCandidateList('2.2, 1.0, 2.2');
  const candidates = buildDistanceCandidates(1.4, parsed);

  assert.deepEqual(candidates, [1.0, 1.4, 2.2]);
});

test('rejects invalid distance candidates', () => {
  assert.throws(() => parseDistanceCandidateList('1.0,,2.0'), /comma-separated/);
  assert.throws(() => parseDistanceCandidateList('1.0,0'), /positive finite/);
  assert.throws(() => parseDistanceCandidateList('1.0,2abc'), /positive finite/);
});

test('selects the closest byte candidate and breaks ties by quality', () => {
  const target = { targetBytes: 1000, targetSsim: 0.94, targetPsnr: 38 };
  const candidates = [
    buildTuningCandidate({ distance: 1.0, bytes: 1120, encodeMs: 5, ssim: 0.96, psnr: 39, ...target }),
    buildTuningCandidate({ distance: 2.0, bytes: 880, encodeMs: 4, ssim: 0.97, psnr: 40, ...target }),
    buildTuningCandidate({ distance: 3.0, bytes: 760, encodeMs: 3, ssim: 0.93, psnr: 36, ...target }),
  ];

  assert.equal(chooseMatchedBytesCandidate(candidates).distance, 2.0);
});

test('selects the closest SSIM candidate and breaks ties by bytes', () => {
  const target = { targetBytes: 1000, targetSsim: 0.94, targetPsnr: 38 };
  const candidates = [
    buildTuningCandidate({ distance: 1.0, bytes: 1300, encodeMs: 5, ssim: 0.955, psnr: 39, ...target }),
    buildTuningCandidate({ distance: 2.0, bytes: 1040, encodeMs: 4, ssim: 0.925, psnr: 37, ...target }),
    buildTuningCandidate({ distance: 3.0, bytes: 900, encodeMs: 3, ssim: 0.90, psnr: 35, ...target }),
  ];

  assert.equal(chooseMatchedQualityCandidate(candidates).distance, 2.0);
});

console.log(`\nTests passed: ${passed}, failed: ${failed}`);
if (failed > 0) {
  process.exit(1);
}
