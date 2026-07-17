const assert = require('node:assert/strict');

const {
  attachPrototypeMethods,
  runTargetBytesFileSearch,
  runTargetBytesSearch,
} = require('../../lib/helpers');

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

test('toFileTargetBytes delegates to native atomic output without returning image data', async () => {
  class Engine {}
  attachPrototypeMethods(Engine)
  const engine = new Engine()
  let received
  engine.toFileTargetBytesNative = async (...args) => {
    received = args
    return {
      bytesWritten: 1234,
      quality: 77,
      budgetMet: true,
      targetBytes: 1500,
      metrics: { totalMs: 1 },
    }
  }

  const result = await engine.toFileTargetBytes('/tmp/output.jpg', 'JPG', {
    targetBytes: 1500,
    minQuality: 40,
    maxQuality: 90,
  })

  assert.deepEqual(received, ['/tmp/output.jpg', 'jpeg', 1500, 40, 90, false, false])
  assert.equal(result.bytesWritten, 1234)
  assert.equal('data' in result, false)
})

test('file search fails clearly when native file output is unavailable', async () => {
  await assert.rejects(
    runTargetBytesFileSearch({}, '/tmp/output.jpg', 'jpeg', { targetBytes: 1000 }),
    /native binding with toFileTargetBytesNative/,
  )
})

test('target-bytes options reject values above the native u32 contract', async () => {
  await assert.rejects(
    runTargetBytesSearch({}, 'jpeg', { targetBytes: 0x1_0000_0000 }),
    /u32 limit \(4294967295\)/,
  )
})
