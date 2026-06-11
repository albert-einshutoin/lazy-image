import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { createUploadOptimizer } from '../browser.js';
import { createUploadOptimizer as createEdgeUploadOptimizer } from '../edge.js';
import edgeExampleHandler from '../examples/edge.mjs';
import { LazyImageWasmError, VERSION } from '../shared.js';

if (typeof globalThis.ImageData !== 'function') {
  globalThis.ImageData = class ImageData {
    constructor(data, width, height) {
      this.data = data;
      this.width = width;
      this.height = height;
    }
  };
}

const require = createRequire(import.meta.url);
const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(packageRoot, '..', '..');
const fixtureRoot = path.join(repoRoot, 'test', 'fixtures');
const packageJson = JSON.parse(await fs.readFile(path.join(packageRoot, 'package.json'), 'utf8'));

function packageFile(packageName, relativePath) {
  return path.join(path.dirname(require.resolve(`${packageName}/package.json`)), relativePath);
}

async function readWasm(packageName, relativePath) {
  return fs.readFile(packageFile(packageName, relativePath));
}

async function loadWasmModules() {
  return {
    jpegDecode: await readWasm('@jsquash/jpeg', 'codec/dec/mozjpeg_dec.wasm'),
    jpegEncode: await readWasm('@jsquash/jpeg', 'codec/enc/mozjpeg_enc.wasm'),
    pngDecode: await readWasm('@jsquash/png', 'codec/pkg/squoosh_png_bg.wasm'),
    resize: await readWasm('@jsquash/resize', 'lib/resize/pkg/squoosh_resize_bg.wasm'),
    webpDecode: await readWasm('@jsquash/webp', 'codec/dec/webp_dec.wasm'),
    webpEncode: await readWasm('@jsquash/webp', 'codec/enc/webp_enc.wasm'),
  };
}

async function fixture(name) {
  return fs.readFile(path.join(fixtureRoot, name));
}

async function test(name, fn) {
  try {
    await fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

const wasmModules = await loadWasmModules();
const optimizer = await createUploadOptimizer({
  wasmModules,
  defaultOutput: 'arrayBuffer',
});

await test('exports the workspace package version', async () => {
  assert.equal(VERSION, packageJson.version);
});

await test('optimizes JPEG input to JPEG with a target byte budget', async () => {
  const input = await fixture('test_100KB_1057x1057.jpg');
  const result = await optimizer.optimizeUpload(input, {
    format: 'jpeg',
    maxWidth: 512,
    maxHeight: 512,
    targetBytes: 35_000,
    minQuality: 35,
    maxQuality: 85,
    output: 'arrayBuffer',
  });

  assert.equal(result.format, 'jpeg');
  assert.ok(result.data instanceof ArrayBuffer);
  assert.ok(result.bytesOut <= 35_000, `bytesOut=${result.bytesOut}`);
  assert.equal(result.budgetMet, true);
  assert.equal(result.metrics.formatIn, 'jpeg');
  assert.equal(result.metrics.metadataStripped, true);
  assert.ok(result.metrics.widthOut <= 512);
  assert.ok(result.metrics.heightOut <= 512);
});

await test('optimizes JPEG input to WebP with best-effort target search', async () => {
  const input = await fixture('test_100KB_1057x1057.jpg');
  const result = await optimizer.optimizeUpload(input, {
    format: 'webp',
    maxWidth: 512,
    maxHeight: 512,
    targetBytes: 30_000,
    minQuality: 35,
    maxQuality: 82,
    output: 'uint8Array',
  });

  assert.equal(result.format, 'webp');
  assert.ok(result.data instanceof Uint8Array);
  assert.equal(result.budgetMet, true);
  assert.equal(result.metrics.formatOut, 'webp');
  assert.ok(result.metrics.encodeMs >= 0);
});

await test('optimizes PNG input to JPEG for upload conversion', async () => {
  const input = await fixture('test_input.png');
  const result = await optimizer.optimizeUpload(input, {
    format: 'jpeg',
    output: 'arrayBuffer',
  });

  assert.equal(result.metrics.formatIn, 'png');
  assert.equal(result.format, 'jpeg');
  assert.ok(result.bytesOut > 0);
});

await test('cover fit produces exact target dimensions', async () => {
  const input = await fixture('test_100KB_1057x1057.jpg');
  const result = await optimizer.optimizeUpload(input, {
    format: 'jpeg',
    fit: 'cover',
    maxWidth: 320,
    maxHeight: 180,
    output: 'arrayBuffer',
  });

  assert.equal(result.metrics.widthOut, 320);
  assert.equal(result.metrics.heightOut, 180);
});

await test('strict target byte policy fails with structured processing error', async () => {
  const input = await fixture('test_38kb_input.jpg');
  await assert.rejects(
    () =>
      optimizer.optimizeUpload(input, {
        format: 'jpeg',
        targetBytes: 10,
        minQuality: 50,
        maxQuality: 51,
        qualityFloorPolicy: 'strict',
        output: 'arrayBuffer',
      }),
    (error) => error instanceof LazyImageWasmError && error.code === 'E201' && error.category === 'Processing'
  );
});

await test('limits.timeoutMs fails with structured processing error', async () => {
  let fakeNow = 0;
  const timeoutOptimizer = await createUploadOptimizer({
    wasmModules,
    defaultOutput: 'arrayBuffer',
    now: () => {
      fakeNow += 5;
      return fakeNow;
    },
  });
  const input = await fixture('test_38kb_input.jpg');

  await assert.rejects(
    () =>
      timeoutOptimizer.optimizeUpload(input, {
        format: 'jpeg',
        output: 'arrayBuffer',
        limits: {
          timeoutMs: 1,
        },
      }),
    (error) => error instanceof LazyImageWasmError && error.code === 'E205' && error.category === 'Processing'
  );
});

await test('unsupported input format fails with E103', async () => {
  await assert.rejects(
    () =>
      optimizer.optimizeUpload(new Uint8Array([1, 2, 3, 4]), {
        format: 'jpeg',
        output: 'arrayBuffer',
      }),
    (error) => error instanceof LazyImageWasmError && error.code === 'E103' && error.category === 'Input'
  );
});

await test('edge entrypoint defaults to ArrayBuffer output', async () => {
  const edgeOptimizer = await createEdgeUploadOptimizer({ wasmModules });
  const input = await fixture('test_input.jpg');
  const result = await edgeOptimizer.optimizeUpload(input, {
    format: 'jpeg',
  });

  assert.ok(result.data instanceof ArrayBuffer);
  assert.equal(result.metrics.runtime, 'edge-isolate');
});

await test('edge optimizer accepts precompiled WebAssembly.Module inputs', async () => {
  const compiledModules = {
    jpegDecode: await WebAssembly.compile(
      await readWasm('@jsquash/jpeg', 'codec/dec/mozjpeg_dec.wasm')
    ),
    jpegEncode: await WebAssembly.compile(
      await readWasm('@jsquash/jpeg', 'codec/enc/mozjpeg_enc.wasm')
    ),
  };

  const edgeOptimizer = await createEdgeUploadOptimizer({ wasmModules: compiledModules });
  const input = await fixture('test_100KB_1057x1057.jpg');
  const result = await edgeOptimizer.optimizeUpload(input, {
    format: 'jpeg',
    maxWidth: 640,
    maxHeight: 640,
    targetBytes: 45_000,
    output: 'arrayBuffer',
  });

  assert.equal(result.metrics.runtime, 'edge-isolate');
  assert.equal(result.format, 'jpeg');
  assert.ok(result.data instanceof ArrayBuffer);
  assert.equal(typeof result.metrics.encodeMs, 'number');
});

await test('edge example handler guards method and maps input errors to HTTP statuses', async () => {
  const env = {
    JSQUASH_JPEG_DECODER: wasmModules.jpegDecode,
    JSQUASH_JPEG_ENCODER: wasmModules.jpegEncode,
    JSQUASH_PNG_DECODER: wasmModules.pngDecode,
    JSQUASH_RESIZE: wasmModules.resize,
    JSQUASH_WEBP_DECODER: wasmModules.webpDecode,
    JSQUASH_WEBP_ENCODER: wasmModules.webpEncode,
  };

  const methodResponse = await edgeExampleHandler.fetch(
    new Request('http://localhost/', { method: 'GET' }),
    env
  );
  assert.equal(methodResponse.status, 405);
  assert.equal(methodResponse.headers.get('allow'), 'POST');

  const input = await fixture('test_100KB_1057x1057.jpg');
  const okResponse = await edgeExampleHandler.fetch(
    new Request('http://localhost/', { method: 'POST', body: input, duplex: 'half' }),
    env
  );
  assert.equal(okResponse.status, 200);
  assert.equal(okResponse.headers.get('content-type'), 'image/jpeg');
  const optimized = new Uint8Array(await okResponse.arrayBuffer());
  assert.equal(
    optimized.byteLength,
    Number(okResponse.headers.get('x-lazy-image-metric-bytes-out'))
  );
  assert.ok(optimized[0] === 0xff && optimized[1] === 0xd8, 'output is JPEG');

  const emptyResponse = await edgeExampleHandler.fetch(
    new Request('http://localhost/', { method: 'POST' }),
    env
  );
  assert.equal(emptyResponse.status, 400);

  const badResponse = await edgeExampleHandler.fetch(
    new Request('http://localhost/', {
      method: 'POST',
      body: new Uint8Array([1, 2, 3, 4]),
      duplex: 'half',
    }),
    env
  );
  assert.equal(badResponse.status, 400);
  assert.match(await badResponse.text(), /^\[E103\]/);
});
