const assert = require('node:assert/strict');

async function main() {
  const { collectPublishedWasmEvidence } = await import('../benchmarks/wasm-published-evidence.mjs');
  await assert.rejects(
    collectPublishedWasmEvidence({ version: '1.3.1', runtime: 'edge' }),
    /required runtime edge is not implemented/
  );
  await assert.rejects(
    collectPublishedWasmEvidence({ version: '', runtime: 'node' }),
    /explicit --version is required/
  );
  console.log('required Wasm runtime and version are fail-closed');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
