const assert = require('node:assert/strict');

async function main() {
  const { validateEdgeDiagnostic } = await import('../benchmarks/wasm-edge-workerd.mjs');
  const base = {
    status: 'FAIL',
    httpStatus: 500,
    error: {
      category: 'CodecError', recoverable: false, isolateId: 'fresh-isolate',
      optimizerCreations: 1,
      recoveryHint: 'Check the .wasm file and DevTools Network. If wasmModules was supplied, check that its module matches the codec.',
    },
  };
  const decoder = { ...base, error: { ...base.error, code: 'E131',
    phase: 'optimizer-initialization', moduleOverride: {
      target: 'jpegDecode', source: 'dynamic-wasm-bytes', bytesHex: '0061736d01000000' },
    message: 'jpeg decoder codec initialization failed: incompatible imports' } };
  assert.equal(validateEdgeDiagnostic('decoder-init', decoder).status, 'PASS');
  const resize = { ...base, error: { ...base.error, code: 'E503',
    phase: 'optimizer-initialization', moduleOverride: { target: 'resize', source: 'jpegDecode' },
    message: 'resize codec initialization failed: incompatible imports' } };
  assert.equal(validateEdgeDiagnostic('resize-init', resize).status, 'PASS');
  const encoder = { ...base, error: { ...base.error, code: 'E300',
    phase: 'optimizer-initialization', moduleOverride: { target: 'webpEncode', source: 'jpegDecode' },
    message: 'webp encoder codec initialization failed: incompatible imports' } };
  assert.equal(validateEdgeDiagnostic('encoder-init', encoder).status, 'PASS');
  const corrupt = { ...base, error: { ...base.error, code: 'E131',
    phase: 'image-processing', moduleOverride: null,
    recoveryHint: 'Check the .wasm request in DevTools Network. If delivery is valid, check the input image for corruption and the codec for a processing failure.',
    message: 'jpeg decoder failed during codec initialization or image decoding: invalid JPEG data' } };
  assert.equal(validateEdgeDiagnostic('corrupt-jpeg', corrupt).status, 'PASS');

  for (const [name, observed] of [
    ['success-response', { ...decoder, status: 'PASS', error: null }],
    ['wrong-code', { ...resize, error: { ...resize.error, code: 'E131' } }],
    ['missing-api-hint', { ...decoder, error: { ...decoder.error, recoveryHint: null } }],
    ['missing-recoverable', { ...decoder, error: { ...decoder.error, recoverable: null } }],
    ['missing-cause', { ...decoder, error: { ...decoder.error,
      message: 'jpeg decoder codec initialization failed: cause unavailable' } }],
    ['wrong-phase', { ...corrupt, error: { ...corrupt.error, phase: 'optimizer-initialization' } }],
    ['wrong-assignment', { ...encoder, error: { ...encoder.error,
      moduleOverride: { target: 'webpEncode', source: 'pngDecode' } } }],
  ]) {
    assert.equal(validateEdgeDiagnostic(name === 'wrong-phase' ? 'corrupt-jpeg' :
      name === 'wrong-code' ? 'resize-init' : name === 'wrong-assignment' ? 'encoder-init' :
        'decoder-init', observed).status, 'FAIL', name);
  }
  console.log('Edge diagnostic validation rejects missing API fields and wrong phase, code, or mapping');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
