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

### Example (Cloudflare Workers / V8 isolate)

`examples/edge.mjs` (shipped in this package) is a small runtime-safe pattern:
it caches one optimizer per isolate, reads `@jsquash` Wasm bindings from
`env.JSQUASH_*` keys, and maps caller-correctable `LazyImageWasmError` values
to HTTP 400. Wasm errors use the native package categories
(`UserError`, `CodecError`, `ResourceLimit`, `InternalBug`); see
[`docs/WASM_PACKAGE_API.md`](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/WASM_PACKAGE_API.md#errors) for the
cross-package code table.

Copy the file into your Worker project and change its relative imports to the
package specifiers (`@alberteinshutoin/lazy-image-wasm/edge` and
`@alberteinshutoin/lazy-image-wasm/shared`), then wire it up:

```js
import { handleOptimizeRequest } from './lazy-image-edge.mjs';

export default {
  async fetch(request, env) {
    return handleOptimizeRequest(request, env);
  },
};
```

Your deployment must provide the codec Wasm as bindings named
`JSQUASH_JPEG_DECODER`, `JSQUASH_JPEG_ENCODER`, `JSQUASH_PNG_DECODER`,
`JSQUASH_RESIZE`, `JSQUASH_WEBP_DECODER`, and `JSQUASH_WEBP_ENCODER`
(`WebAssembly.Module`, `ArrayBuffer`, or `Uint8Array` values).

### Edge and browser constraints

- Supported formats stay limited to `jpeg` and `webp` output, and input is currently
  `jpeg` / `png` / `webp`.
- No filesystem, native filesystem APIs, or Node built-ins are available.
- `@jsquash/*` codec Wasm blobs are loaded from your deployment/runtime and must be
  provided via `wasmModules`.
- If you use `output: 'blob'` in runtimes that do not expose `Blob`, the API returns
  an error and you should request `arrayBuffer` or `uint8Array` instead.

Bundle considerations:

- Keep this package behind lazy-loading where possible and avoid eagerly shipping the
  full browser/resize codec set for pages that do not optimize uploads.
- Run `npm run test:bench:wasm` locally and inspect generated artifacts to decide
  whether full `resize`/`webp` codec loading is justified for your target flow.
- For browser/Edge evidence, collect both bundle/gzip size and encode-metrics before
  publishing any performance claim.

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
