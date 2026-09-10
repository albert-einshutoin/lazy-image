const assert = require('node:assert/strict');
const { selectMatches } = require('../benchmarks/quality-matched.bench');

const candidates = [
  { quality: 20, bytes: 110, ssim: 0.91 },
  { quality: 40, bytes: 90, ssim: 0.92 },
  { quality: 60, bytes: 100, ssim: 0.94 },
  { quality: 80, bytes: 120, ssim: 0.99 },
];
// Non-monotonic size/quality must not make a binary search skip the best candidate.
assert.deepEqual(selectMatches(candidates, 0.92, 100), {
  qualityMatched: candidates[1], byteMatched: candidates[2],
});
assert.deepEqual(selectMatches(candidates, 1, 89), {
  qualityMatched: null, byteMatched: null,
});
assert.deepEqual(selectMatches([], 0.9, 100), { qualityMatched: null, byteMatched: null });
assert.throws(() => selectMatches([{ bytes: 1, ssim: NaN }], 0.9, 100), /candidate/);
assert.throws(() => selectMatches(candidates, NaN, 100), /target/);
assert.throws(() => selectMatches(candidates, 0.9, 0), /target/);
console.log('quality-matched selection checks passed');
