const assert = require('node:assert/strict');

const { runTargetBytesSearch } = require('../../lib/helpers');

async function runTest(name, fn) {
  try {
    await fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

async function run() {
  await runTest('runTargetBytesSearch uses native toBufferTargetBytesNative when available', async () => {
    const nativeResult = {
      data: Buffer.from('abc'),
      quality: 87,
      bytesOut: 3,
      budgetMet: true,
      targetBytes: 2000,
      metrics: { totalMs: 12 },
    };

    let called = false;
    const engine = {
      toBufferTargetBytesNative: async (...args) => {
        called = true;
        assert.equal(args[0], 'jpeg');
        assert.equal(args[1], 2000);
        assert.equal(args[2], 40);
        assert.equal(args[3], 90);
        assert.equal(args[4], true);
        assert.equal(args[5], false);
        return nativeResult;
      },
    };

    const result = await runTargetBytesSearch(engine, 'jpeg', {
      targetBytes: 2000,
      minQuality: 40,
      maxQuality: 90,
      fastMode: true,
      qualityFloorPolicy: 'best-effort',
    });

    assert.equal(called, true);
    assert.deepEqual(result, nativeResult);
  });

  await runTest('runTargetBytesSearch rejects when toBufferTargetBytesNative is missing', async () => {
    const engine = {
      toBufferWithMetrics: async () => ({
        data: Buffer.from('noop'),
        metrics: { bytesOut: 999 },
      }),
    };

    await assert.rejects(
      () =>
        runTargetBytesSearch(engine, 'jpeg', {
          targetBytes: 2000,
          minQuality: 40,
          maxQuality: 90,
        }),
      /Reinstall @alberteinshutoin\/lazy-image so the JS and native versions match/,
      'runTargetBytesSearch should explain native-binding mismatch explicitly',
    );
  });
}

run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
