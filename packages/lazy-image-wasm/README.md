# @alberteinshutoin/lazy-image-wasm

Browser and Edge upload-preflight optimizer for lazy-image policy APIs.

This package is the first Wasm MVP. It uses jSquash Wasm codecs as the low-level
browser/V8 codec adapter while lazy-image owns the upload policy, target-byte
search, metrics, and structured errors.

## Browser

```js
import { optimizeUpload } from '@alberteinshutoin/lazy-image-wasm/browser';

const result = await optimizeUpload(file, {
  format: 'webp',
  maxWidth: 1600,
  maxHeight: 1600,
  targetBytes: 350_000,
  output: 'blob',
});
```

## Edge

Edge runtimes often need explicit Wasm module loading. Pass compiled
`WebAssembly.Module`, `ArrayBuffer`, or `Uint8Array` values via `wasmModules`.

```js
import { createUploadOptimizer } from '@alberteinshutoin/lazy-image-wasm/edge';

const optimizer = await createUploadOptimizer({ wasmModules });
const result = await optimizer.optimizeUpload(new Uint8Array(await request.arrayBuffer()), {
  format: 'jpeg',
  maxWidth: 1200,
  maxHeight: 1200,
  targetBytes: 250_000,
});
```

## Worker

```js
import { createUploadWorkerHandler } from '@alberteinshutoin/lazy-image-wasm/worker';

self.addEventListener('message', createUploadWorkerHandler());
```

The MVP supports JPEG, PNG, and WebP input; JPEG and WebP output; metadata
stripping by default; best-effort and strict target-byte policies; and metrics
compatible with the native lazy-image vocabulary where possible. Resize policies
support `inside`, `cover`, and `fill`; `limits.timeoutMs` is enforced between
input, decode, resize, and encode stages as a best-effort checkpoint.
