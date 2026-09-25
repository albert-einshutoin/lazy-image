const assert = require('node:assert/strict');
const { createHash } = require('node:crypto');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');

const files = [
  ['@jsquash/jpeg', 'codec/dec/mozjpeg_dec.wasm'],
  ['@jsquash/jpeg', 'codec/enc/mozjpeg_enc.wasm'],
  ['@jsquash/png', 'codec/pkg/squoosh_png_bg.wasm'],
  ['@jsquash/resize', 'lib/resize/pkg/squoosh_resize_bg.wasm'],
  ['@jsquash/webp', 'codec/enc/webp_enc.wasm'],
];

async function main() {
  const { runNode } = await import('../benchmarks/wasm-published-evidence.mjs');
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-node-diagnostic-test-'));
  try {
    for (const [codec, relative] of files) {
      const file = path.join(directory, 'node_modules', codec, relative);
      await fs.mkdir(path.dirname(file), { recursive: true });
      await fs.writeFile(file, Buffer.from([1]));
    }
    const browserImport = path.join(directory, 'browser.mjs');
    await fs.writeFile(browserImport, `
      export async function createUploadOptimizer({ wasmModules }) {
        if (wasmModules.resize[0] === 0) {
          throw Object.assign(new Error('resize codec initialization failed: invalid Wasm bytes'), {
            code: 'E503', category: 'CodecError', recoverable: false,
            recoveryHint: 'Check the supplied resize wasmModules value.'
          });
        }
        return { async optimizeUpload() {
          throw Object.assign(new Error('jpeg decoder failed during image decoding'), {
            code: 'E131', category: 'CodecError', recoverable: false,
            recoveryHint: 'Check the input image after confirming Wasm delivery.'
          });
        } };
      }
    `);
    const input = path.join(directory, 'valid.jpg');
    await fs.writeFile(input, Buffer.from([0xff, 0xd8, 0xff, 0xd9]));
    const packageInfo = { browserImport };
    const cases = [{ id: 'jpeg-webp', input,
      options: { format: 'webp', output: 'arrayBuffer' } }];
    for (const [name, code, stage, hint] of [
      ['corrupt-jpeg', 'E131', 'image-processing', /input image/],
      ['resize-init', 'E503', 'optimizer-initialization', /resize wasmModules/],
    ]) {
      const stages = [];
      const result = await runNode(packageInfo, directory, cases,
        { diagnosticCase: name, onStage: (value) => stages.push(value), artifactDirectory: directory });
      assert.equal(result.diagnostic.imageProcessingOutcome, 'FAIL');
      assert.equal(result.diagnostic.failureStage, stage);
      assert.equal(result.diagnostic.apiError.code, code);
      assert.equal(result.diagnostic.apiError.category, 'CodecError');
      assert.equal(result.diagnostic.apiError.recoverable, false);
      assert.match(result.diagnostic.apiError.recoveryHint, hint);
      assert.equal(result.diagnosticValidation.status, 'PASS');
      assert.equal(stages.at(-1), stage);
      assert.equal(result.moduleAssignments.length, 5);
      if (name === 'resize-init') {
        const resize = result.moduleAssignments.find((entry) => entry.key === 'resize');
        assert.equal(resize.source, 'explicit-invalid-bytes');
        assert.equal(resize.sha256, createHash('sha256').update(Buffer.from([0])).digest('hex'));
      } else {
        const bytes = await fs.readFile(path.join(directory, 'wasm-node-corrupt-jpeg-input.jpg'));
        assert.deepEqual(bytes, Buffer.from([0xff, 0xd8, 0xff, 0, 0, 0]));
        assert.equal(result.diagnostic.input.sha256,
          createHash('sha256').update(bytes).digest('hex'));
      }
    }
    console.log('Node diagnostic collector preserves initialization and decode API errors');
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
