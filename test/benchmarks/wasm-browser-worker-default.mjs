import { createUploadWorkerHandler } from '@alberteinshutoin/lazy-image-wasm/worker';

self.addEventListener('message', createUploadWorkerHandler());
self.postMessage({
  type: 'setup',
  workerScope: self.constructor.name,
  hasNativeImageData: typeof ImageData === 'function',
  hasWebAssembly: typeof WebAssembly?.compile === 'function',
  assetBytes: {},
});
