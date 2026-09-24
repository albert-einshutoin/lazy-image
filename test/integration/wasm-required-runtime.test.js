const assert = require('node:assert/strict');
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
  console.log('required Wasm runtime and version are fail-closed');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
