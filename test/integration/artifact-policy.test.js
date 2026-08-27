const assert = require('node:assert/strict');
const { compileArtifactPlan } = require('../../lib/artifact-policy');

const SOURCE = Object.freeze({
  sha256: 'a'.repeat(64),
  bytes: 1024,
  format: 'jpeg',
  width: 2000,
  height: 1200,
  hasAlpha: false,
  isAnimated: false,
  orientationKnown: true,
  orientation: null,
});
const COMPILER_FINGERPRINT = 'b'.repeat(64);

function assertPolicyError(run, message) {
  assert.throws(run, (error) => {
    assert.equal(error.code, 'LAZY_IMAGE_USER_ERROR');
    assert.equal(error.errorCode, 'E400');
    assert.equal(error.errorCategory, 'UserError');
    assert.equal(typeof error.recoveryHint, 'string');
    assert.match(error.message, message);
    return true;
  });
}

const plan = compileArtifactPlan(SOURCE, {}, COMPILER_FINGERPRINT);
assert.deepEqual(plan.policy, {
  preset: 'publicUpload',
  widths: [320, 640, 1280],
  formats: ['webp'],
  budgets: [],
  placeholder: true,
});
assert.deepEqual(plan.artifacts.map(({ id }) => id), ['320/webp', '640/webp', '1280/webp']);
assert.equal(plan.delivery.srcsets[0].value, 'image-320.webp 320w, image-640.webp 640w, image-1280.webp 1280w');
assert.equal(plan.placeholder.path, 'placeholder.webp');
assert(Object.isFrozen(plan));
assert(Object.isFrozen(plan.policy.widths));

const input = {
  preset: 'publicUpload',
  widths: [1280, 320, 320, 768],
  formats: ['avif', 'jpeg', 'webp'],
  budgets: [{ width: 768, format: 'webp', maxBytes: 50_000 }],
  placeholder: false,
};
const originalInput = structuredClone(input);
const canonical = compileArtifactPlan(SOURCE, input, COMPILER_FINGERPRINT);
assert.deepEqual(input, originalInput);
assert.deepEqual(canonical.policy.widths, [320, 768, 1280]);
assert.deepEqual(canonical.policy.formats, ['jpeg', 'webp', 'avif']);
assert.deepEqual(canonical.artifacts.map(({ id }) => id), [
  '320/jpeg', '768/jpeg', '1280/jpeg',
  '320/webp', '768/webp', '1280/webp',
  '320/avif', '768/avif', '1280/avif',
]);
assert.equal(canonical.artifacts.find(({ id }) => id === '768/webp').targetBytes, 50_000);
assert.equal(canonical.placeholder, null);

const duplicateWidths = compileArtifactPlan(
  SOURCE,
  { widths: Array(9).fill(320) },
  COMPILER_FINGERPRINT,
);
assert.deepEqual(duplicateWidths.policy.widths, [320]);
const duplicateFormats = compileArtifactPlan(
  SOURCE,
  { formats: ['avif', 'jpeg', 'jpeg', 'webp'] },
  COMPILER_FINGERPRINT,
);
assert.deepEqual(duplicateFormats.policy.formats, ['jpeg', 'webp', 'avif']);

const withoutEnlargement = compileArtifactPlan(
  { ...SOURCE, width: 500, height: 300 },
  { widths: [1000] },
  COMPILER_FINGERPRINT,
);
assert.deepEqual(withoutEnlargement.policy.widths, [500]);

const rotated = compileArtifactPlan(
  { ...SOURCE, width: 1200, height: 800, orientation: 6 },
  { widths: [900] },
  COMPILER_FINGERPRINT,
);
assert.equal(rotated.source.displayWidth, 800);
assert.equal(rotated.source.displayHeight, 1200);
assert.deepEqual(rotated.policy.widths, [800]);
assert.equal(compileArtifactPlan({ ...SOURCE, format: 'jpg' }, {}, COMPILER_FINGERPRINT).source.detectedFormat, 'jpeg');

const same = compileArtifactPlan(SOURCE, {}, COMPILER_FINGERPRINT);
assert.deepEqual(same, plan);
assert.notEqual(
  compileArtifactPlan(SOURCE, { placeholder: false }, COMPILER_FINGERPRINT).cacheKey,
  plan.cacheKey,
);
assert.notEqual(compileArtifactPlan(SOURCE, {}, 'c'.repeat(64)).cacheKey, plan.cacheKey);
assert(!JSON.stringify(plan).includes(process.cwd()));

const invalidCases = [
  [{ widths: [15] }, /widths/],
  [{ widths: Array.from({ length: 9 }, (_, index) => index + 16) }, /at most 8/],
  [{ formats: ['png'] }, /formats/],
  [{ unknown: true }, /unknown policy key/],
  [{ budgets: [{ width: 999, format: 'webp', maxBytes: 1 }] }, /budget target/],
  [{ budgets: [
    { width: 320, format: 'webp', maxBytes: 1 },
    { width: 320, format: 'webp', maxBytes: 2 },
  ] }, /duplicate budget/],
  [{ budgets: [{ width: 320, format: 'webp', maxBytes: 32 * 1024 * 1024 + 1 }] }, /maxBytes/],
];
for (const [policy, message] of invalidCases) {
  assertPolicyError(() => compileArtifactPlan(SOURCE, policy, COMPILER_FINGERPRINT), message);
}

assertPolicyError(
  () => compileArtifactPlan({ ...SOURCE, hasAlpha: true }, { formats: ['jpeg'] }, COMPILER_FINGERPRINT),
  /alpha.*jpeg/i,
);
assertPolicyError(
  () => compileArtifactPlan({ ...SOURCE, isAnimated: true }, {}, COMPILER_FINGERPRINT),
  /animated/i,
);
assertPolicyError(
  () => compileArtifactPlan({ ...SOURCE, hasAlpha: undefined }, {}, COMPILER_FINGERPRINT),
  /hasAlpha/,
);
for (const [source, message] of [
  [{ ...SOURCE, isAnimated: undefined }, /isAnimated/],
  [{ ...SOURCE, orientationKnown: false }, /orientationKnown/],
  [{ ...SOURCE, orientation: undefined }, /source.orientation/],
  [{ ...SOURCE, sha256: 'invalid' }, /sha256/],
  [{ ...SOURCE, sha256: { toString: () => 'a'.repeat(64), toJSON: () => 'leak' } }, /sha256/],
  [{ ...SOURCE, bytes: 32 * 1024 * 1024 + 1 }, /source.bytes/],
  [{ ...SOURCE, format: 'avif' }, /source.format/],
  [{ ...SOURCE, width: 10_000, height: 10_000 }, /pixels/],
  [{ ...SOURCE, path: '/private/input.jpg' }, /unknown source key/],
]) {
  assertPolicyError(() => compileArtifactPlan(source, {}, COMPILER_FINGERPRINT), message);
}
assertPolicyError(
  () => compileArtifactPlan(SOURCE, {}, 'not-a-fingerprint'),
  /compilerFingerprint/,
);
assertPolicyError(
  () => compileArtifactPlan(SOURCE, { widths: [, 32] }, COMPILER_FINGERPRINT),
  /widths/,
);
assertPolicyError(
  () => compileArtifactPlan(SOURCE, { widths: Array(65).fill(320) }, COMPILER_FINGERPRINT),
  /at most 64 raw entries/,
);
assertPolicyError(
  () => compileArtifactPlan(SOURCE, { formats: Array(13).fill('webp') }, COMPILER_FINGERPRINT),
  /at most 12 raw entries/,
);
for (const [policy, message] of [
  [{ preset: null }, /policy\.preset/],
  [{ widths: null }, /widths/],
  [{ formats: null }, /formats/],
  [{ budgets: null }, /budgets/],
  [{ placeholder: null }, /placeholder/],
]) {
  assertPolicyError(() => compileArtifactPlan(SOURCE, policy, COMPILER_FINGERPRINT), message);
}
assertPolicyError(
  () => compileArtifactPlan(SOURCE, {}, { toString: () => 'b'.repeat(64), toJSON: () => 'leak' }),
  /compilerFingerprint/,
);

console.log('artifact policy contract passed');
