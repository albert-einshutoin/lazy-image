import { createUploadWorkerHandler } from '@alberteinshutoin/lazy-image-wasm/worker';

const codecAssets = {
  jpegDecode: '/assets/mozjpeg_dec.wasm',
  jpegEncode: '/assets/mozjpeg_enc.wasm',
  pngDecode: '/assets/squoosh_png_bg.wasm',
  resize: '/assets/squoosh_resize_bg.wasm',
  webpEncode: '/assets/webp_enc.wasm',
};

let setup;
self.addEventListener('message', async (event) => {
  try {
    if (!setup) {
      setup = (async () => {
        const start = performance.now();
        const entries = await Promise.all(Object.entries(codecAssets).map(async ([name, url]) => {
          const response = await fetch(url);
          if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
          return [name, await response.arrayBuffer()];
        }));
        self.postMessage({ type: 'setup', assetFetchMs: performance.now() - start,
          workerScope: self.constructor.name, hasNativeImageData: typeof ImageData === 'function',
          hasWebAssembly: typeof WebAssembly?.compile === 'function',
          assetBytes: Object.fromEntries(entries.map(([name, bytes]) => [name, bytes.byteLength])) });
        return createUploadWorkerHandler({ wasmModules: Object.fromEntries(entries), defaultOutput: 'arrayBuffer' });
      })();
    }
    const handler = await setup;
    await handler(event);
  } catch (error) {
    self.postMessage({ id: event.data?.id, ok: false,
      error: { name: error.name, message: error.message, code: error.code, stack: error.stack } });
  }
});
