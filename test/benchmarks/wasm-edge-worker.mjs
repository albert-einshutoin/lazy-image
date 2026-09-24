import { createUploadOptimizer } from '@alberteinshutoin/lazy-image-wasm/edge';
import jpegDecode from './mozjpeg_dec.wasm';
import jpegEncode from './mozjpeg_enc.wasm';
import pngDecode from './squoosh_png_bg.wasm';
import resize from './squoosh_resize_bg.wasm';
import webpEncode from './webp_enc.wasm';

const wasmModules = { jpegDecode, jpegEncode, pngDecode, resize, webpEncode };
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
    try {
      const options = JSON.parse(request.headers.get('x-lazy-options'));
      if (!optimizerPromise) {
        optimizerCreations++;
        optimizerPromise = createUploadOptimizer({ wasmModules, defaultOutput: 'arrayBuffer' })
          .catch((error) => { optimizerPromise = undefined; throw error; });
      }
      const optimizer = await optimizerPromise;
      const result = await optimizer.optimizeUpload(await request.arrayBuffer(), options);
      return new Response(result.data, { headers: {
        'content-type': options.format === 'webp' ? 'image/webp' : 'image/jpeg',
        'x-lazy-result': JSON.stringify({ bytesOut: result.bytesOut, budgetMet: result.budgetMet,
          targetBytes: result.targetBytes, metrics: result.metrics,
          isolateId: currentIsolateId(), optimizerCreations }),
      } });
    } catch (error) {
      return Response.json({ code: error.code ?? null, category: error.category ?? null,
        message: error.message, isolateId: currentIsolateId(), optimizerCreations },
      { status: error.code === 'E502' ? 422 : 500 });
    }
  },
};
