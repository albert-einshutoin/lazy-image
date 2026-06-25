const assert = require('node:assert/strict');

const { runTargetBytesSearch } = require('../../lib/helpers');

function test(name, fn) {
  fn()
    .then(() => console.log(`ok - ${name}`))
    .catch((error) => {
      console.error(`not ok - ${name}`);
      throw error;
    });
}

test('runTargetBytesSearch uses toBufferTargetBytesNative and returns normalized fields', async () => {
  let calledArgs;
  const expected = {
    data: Buffer.from('ok'),
    quality: 72,
    bytesOut: 120,
    targetBytes: 2000,
    budgetMet: true,
    metrics: { totalMs: 1.5 },
  };

  const engine = {
    toBufferTargetBytesNative: async (format, targetBytes, minQuality, maxQuality, fastMode, strict) => {
      calledArgs = { format, targetBytes, minQuality, maxQuality, fastMode, strict };
      return expected;
    },
  };

  const result = await runTargetBytesSearch(engine, 'jPeG', {
    targetBytes: 2000,
    minQuality: 40,
    maxQuality: 90,
    fastMode: true,
    qualityFloorPolicy: 'strict',
  });

  assert.deepStrictEqual(result, expected);
  assert.deepStrictEqual(calledArgs, {
    format: 'jpeg',
    targetBytes: 2000,
    minQuality: 40,
    maxQuality: 90,
    fastMode: true,
    strict: true,
  });
});

test('runTargetBytesSearch accepts jpg and defaults strict policy to false', async () => {
  let calledArgs;
  const expected = {
    data: Buffer.from('ok'),
    quality: 70,
    bytesOut: 110,
    budgetMet: false,
    targetBytes: 1500,
    metrics: null,
  };

  const engine = {
    toBufferTargetBytesNative: async (format, targetBytes, minQuality, maxQuality, fastMode, strict) => {
      calledArgs = { format, targetBytes, minQuality, maxQuality, fastMode, strict };
      return expected;
    },
  };

  const result = await runTargetBytesSearch(engine, 'jpg', {
    targetBytes: 1500,
    minQuality: 30,
    maxQuality: 95,
  });

  assert.deepStrictEqual(result, expected);
  assert.deepStrictEqual(calledArgs, {
    format: 'jpeg',
    targetBytes: 1500,
    minQuality: 30,
    maxQuality: 95,
    fastMode: false,
    strict: false,
  });
});

test('runTargetBytesSearch throws explicit guidance when native target-bytes method is missing', async () => {
  await assert.rejects(
    runTargetBytesSearch({}, 'jpeg', {
      targetBytes: 2000,
      minQuality: 40,
      maxQuality: 90,
    }),
    /Reinstall @alberteinshutoin\/lazy-image/, 'should instruct reinstall when native method is absent',
  );
});
