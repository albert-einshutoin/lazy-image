import { createUploadOptimizer } from '@alberteinshutoin/lazy-image-wasm/edge';
import jpegDecode from './mozjpeg_dec.wasm';
import jpegEncode from './mozjpeg_enc.wasm';
import pngDecode from './squoosh_png_bg.wasm';
import resize from './squoosh_resize_bg.wasm';
import webpEncode from './webp_enc.wasm';

const wasmModules = { jpegDecode, jpegEncode, pngDecode, resize, webpEncode };
const diagnosticOverrides = {
  // The published JPEG codec starts its Emscripten module lazily, so a valid
  // but wrong Module fails at decode. workerd rejects dynamic compilation of
  // valid bytes during optimizer creation, exercising the public init error.
  'decoder-init': { target: 'jpegDecode', source: 'dynamic-wasm-bytes', bytesHex: '0061736d01000000' },
  'resize-init': { target: 'resize', source: 'jpegDecode' },
  'encoder-init': { target: 'webpEncode', source: 'jpegDecode' },
};
let isolateId;
function currentIsolateId() {
  return isolateId ??= crypto.randomUUID();
}
let optimizerPromise;
let optimizerCreations = 0;

export default {
  async fetch(request) {
    const pathname = new URL(request.url).pathname;
    if (pathname === '/health') {
      return Response.json({ isolateId: currentIsolateId(), optimizerCreations,
        imageDataType: typeof ImageData,
        wasmModules: Object.fromEntries(Object.entries(wasmModules)
          .map(([name, module]) => [name, module instanceof WebAssembly.Module])) });
    }
    if (pathname === '/clock') {
      const performanceBefore = performance.now();
      const dateBefore = Date.now();
      let value = 0;
      for (let index = 0; index < 2_000_000; index++) value += index;
      return Response.json({ performanceBefore, performanceAfter: performance.now(),
        dateBefore, dateAfter: Date.now(), value });
    }
    if (pathname !== '/process' || request.method !== 'POST') {
      return new Response('POST /process', { status: 405 });
    }
    let phase = 'request-options';
    let moduleOverride = null;
    try {
      const options = JSON.parse(request.headers.get('x-lazy-options'));
      const diagnosticCase = request.headers.get('x-lazy-edge-diagnostic');
      if (diagnosticCase && diagnosticCase !== 'corrupt-jpeg') {
        moduleOverride = diagnosticOverrides[diagnosticCase];
        if (!moduleOverride) throw new Error(`unsupported Edge diagnostic case: ${diagnosticCase}`);
      }
      const modules = moduleOverride
        ? { ...wasmModules, [moduleOverride.target]: moduleOverride.bytesHex
          ? Uint8Array.from(moduleOverride.bytesHex.match(/../g), (byte) => parseInt(byte, 16))
          : wasmModules[moduleOverride.source] }
        : wasmModules;
      phase = 'optimizer-initialization';
      if (!optimizerPromise) {
        optimizerCreations++;
        optimizerPromise = createUploadOptimizer({ wasmModules: modules, defaultOutput: 'arrayBuffer' })
          .catch((error) => { optimizerPromise = undefined; throw error; });
      }
      const optimizer = await optimizerPromise;
      phase = 'image-processing';
      const result = await optimizer.optimizeUpload(await request.arrayBuffer(), options);
      return new Response(result.data, { headers: {
        'content-type': options.format === 'webp' ? 'image/webp' : 'image/jpeg',
        'x-lazy-result': JSON.stringify({ bytesOut: result.bytesOut, budgetMet: result.budgetMet,
          targetBytes: result.targetBytes, metrics: result.metrics,
          isolateId: currentIsolateId(), optimizerCreations }),
      } });
    } catch (error) {
      return Response.json({ code: error?.code ?? null, category: error?.category ?? null,
        recoverable: error?.recoverable ?? null, message: error?.message ?? null,
        recoveryHint: error?.recoveryHint ?? null, phase, moduleOverride,
        isolateId: currentIsolateId(), optimizerCreations },
      { status: error?.code === 'E502' ? 422 : 500 });
    }
  },
};
