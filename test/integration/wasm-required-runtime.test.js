const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { RUNTIME_FILTERS } = require('../benchmarks/wasm-upload-comparison.bench');

async function main() {
  const { collectPublishedWasmEvidence } = await import('../benchmarks/wasm-published-evidence.mjs');
  assert.deepEqual(RUNTIME_FILTERS.get('all'), ['node-wasm', 'browser-worker', 'edge-isolate']);
  assert.deepEqual(RUNTIME_FILTERS.get('edge'), ['edge-isolate']);
  await assert.rejects(collectPublishedWasmEvidence({ version: '1.3.1', runtime: 'unknown' }),
    /required runtime unknown is not implemented/);
  await assert.rejects(
    collectPublishedWasmEvidence({ version: '', runtime: 'node' }),
    /explicit --version is required/
  );
  await assert.rejects(
    collectPublishedWasmEvidence({ version: '1.3.1', runtime: 'browser', browserLoad: 'unknown' }),
    /unsupported browser load mode/
  );
  await assert.rejects(
    collectPublishedWasmEvidence({ version: '1.3.1', runtime: 'browser', withholdWasm: 'mozjpeg_dec.wasm' }),
    /--withhold-wasm requires a default-load browser run/
  );
  const originalOverride = process.env.ESBUILD_BINARY_PATH;
  try {
    process.env.ESBUILD_BINARY_PATH = '/definitely/not/the/installed/esbuild';
    await assert.rejects(
      collectPublishedWasmEvidence({ version: '1.3.1', runtime: 'edge' }),
      /external esbuild override is unsupported: unset ESBUILD_BINARY_PATH/
    );
    const report = JSON.parse(fs.readFileSync(path.resolve(__dirname,
      '../../artifacts/benchmark/wasm-published-evidence.json')));
    assert.equal(report.verdict, 'FAIL');
    assert.match(report.error.message, /ESBUILD_BINARY_PATH/);
  } finally {
    if (originalOverride === undefined) delete process.env.ESBUILD_BINARY_PATH;
    else process.env.ESBUILD_BINARY_PATH = originalOverride;
  }
  console.log('required Wasm runtime and version are fail-closed');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
