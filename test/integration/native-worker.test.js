const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { Worker, isMainThread, parentPort, workerData } = require('node:worker_threads');

const BINDING = path.resolve(__dirname, '../../index.js');
const INPUT = path.resolve(__dirname, '../fixtures/test_input.jpg');
const LARGE_INPUT = path.resolve(__dirname, '../fixtures/test_100KB_1057x1057.jpg');

async function encode(inputPath, format = 'jpeg') {
  const { ImageEngine, inspect } = require(BINDING);
  const input = fs.readFileSync(inputPath);
  const output = await ImageEngine.from(input).toBuffer(format, format === 'avif' ? 60 : 80);
  assert.ok(Buffer.isBuffer(output) && output.length > 0);
  const metadata = inspect(output);
  assert.equal(metadata.format, format);
  assert.ok(metadata.width > 0 && metadata.height > 0);
  return output.length;
}

async function runWorker() {
  if (workerData.terminateAfterStart) {
    const pending = encode(LARGE_INPUT, 'avif');
    parentPort.postMessage({ type: 'started' });
    await pending;
  } else {
    const bytes = await encode(INPUT);
    parentPort.postMessage({ type: 'done', bytes });
  }
}

function waitForWorker(terminateAfterStart = false) {
  return new Promise((resolve, reject) => {
    const worker = new Worker(__filename, { workerData: { terminateAfterStart } });
    let completed = false;
    let started = false;
    worker.on('message', message => {
      if (message.type === 'started') {
        started = true;
        worker.terminate().catch(reject);
      } else if (message.type === 'done') {
        assert.ok(message.bytes > 0);
        completed = true;
      }
    });
    worker.once('error', reject);
    worker.once('exit', code => {
      try {
        if (terminateAfterStart) {
          assert.ok(started, 'native async work must start before termination');
          assert.equal(code, 1, 'explicit termination has its own exit status');
        } else {
          assert.ok(completed, 'completed image output must be reported');
          assert.equal(code, 0, 'worker must exit naturally');
        }
        resolve();
      } catch (error) {
        reject(error);
      }
    });
  });
}

async function runChild(scenario) {
  if (scenario === 'main-and-worker') {
    await encode(INPUT);
    for (let i = 0; i < 3; i += 1) await waitForWorker();
  } else if (scenario === 'worker-only') {
    await waitForWorker();
  } else if (scenario === 'terminate-after-async') {
    await waitForWorker(true);
  } else {
    throw new Error(`unknown scenario: ${scenario}`);
  }
}

if (!isMainThread) {
  runWorker().catch(error => {
    console.error(error);
    process.exitCode = 1;
  });
} else if (process.argv[2] === '--child') {
  runChild(process.argv[3]).catch(error => {
    console.error(error);
    process.exitCode = 1;
  });
} else {
  for (const scenario of ['worker-only', 'main-and-worker', 'terminate-after-async']) {
    const env = { ...process.env };
    delete env.NAPI_RS_NATIVE_LIBRARY_PATH;
    const result = spawnSync(process.execPath, [__filename, '--child', scenario], {
      env,
      encoding: 'utf8',
      timeout: 90_000,
    });
    assert.ifError(result.error);
    assert.equal(result.signal, null, `${scenario}: signal ${result.signal}\n${result.stderr}`);
    assert.equal(result.status, 0, `${scenario}: exit ${result.status}\n${result.stderr}`);
    console.log(`native Worker ${scenario}: PASS`);
  }
}
