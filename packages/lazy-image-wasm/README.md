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
- In a browser module Worker, the published codecs can resolve deployed Wasm
  files themselves as described below. Edge isolates in the verified workerd
  setup use `wasmModules` with static `WebAssembly.Module` imports.
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

The package can use the codecs' normal Wasm loader in a
Chrome module Worker. Install the package and a bundler, then put the Worker
bundle and the **unchanged** published codec Wasm files at the same URL level.
The codec loader resolves Wasm with `new URL(..., import.meta.url)`; the tested
esbuild ESM bundle emits those URLs relative to `worker.js`.

```bash
npm install @alberteinshutoin/lazy-image-wasm@1.4.1
npm install --save-dev esbuild@0.25.10
./node_modules/.bin/esbuild worker.mjs --bundle --format=esm --platform=browser --target=es2022 --outfile=dist/worker.js
find node_modules/@jsquash/{jpeg,png,resize,webp} -name '*.wasm' -exec cp {} dist/ \;
```

`worker.mjs`:

```js
import { createUploadWorkerHandler } from '@alberteinshutoin/lazy-image-wasm/worker';

self.addEventListener('message', createUploadWorkerHandler());
```

Put the page in `dist/index.html` and serve `dist/` as the HTTP document root
(for example, `python3 -m http.server 8080 --directory dist`). Then
`dist/worker.js` is available at `/worker.js` and the copied Wasm files at
`/<name>.wasm`. Call the Worker from that page:

```js
const file = document.querySelector('input[type=file]').files[0];
const worker = new Worker('/worker.js', { type: 'module' });
worker.onmessage = ({ data }) => {
  if (!data.ok) {
    console.error(data.error.code, data.error.message, data.error.recoveryHint);
    return;
  }
  const output = new Blob([data.result.data], { type: 'image/webp' });
  // Use or upload output.
};
worker.postMessage({ id: 1, input: await file.arrayBuffer(), options: {
  format: 'webp', maxWidth: 1600, maxHeight: 1600, targetBytes: 500_000,
  output: 'arrayBuffer',
} });
```

Serve the generated JS as JavaScript and `.wasm` as `application/wasm` over
HTTP(S). In the tested Chrome 153.0.8010.53 setup, the JPEG→WebP path fetched
`mozjpeg_dec.wasm`, `squoosh_resize_bg.wasm`, and **`webp_enc_simd.wasm`**;
PNG→JPEG additionally fetched `squoosh_png_bg.wasm` and `mozjpeg_enc.wasm`.
The copy command places all nine published codec Wasm files, allowing other
supported paths and codec variants to resolve. Missing required Wasm fails
image processing. The reproducible HTTP server, exact asset hashes, and output
checks for v1.3.1 are in [the browser verification record](https://github.com/albert-einshutoin/lazy-image/blob/main/docs/history/WASM_1.3.1_BROWSER_DEFAULT_VERIFICATION.md).

If the Worker returns `E131` for JPEG decoding, inspect its `message` and
`recoveryHint` before changing the image. In the setup above, confirm
`dist/mozjpeg_dec.wasm` exists, then use Chrome DevTools **Network** to check
the `.wasm` request URL and HTTP status. The response must contain Wasm bytes,
not a 404 page or HTML. If Wasm delivery is correct, check the JPEG for
corruption or a codec processing failure. `E503` during resizing follows the
same check for `squoosh_resize_bg.wasm`. `E300` during WebP encoding follows
the same check for the variant actually requested (`webp_enc_simd.wasm` in the
verified Chrome setup). The error does **not** claim an HTTP status or URL it
cannot observe; Network provides that evidence. The Worker returns the same
`code`, `category`, `recoverable`, `message`, and `recoveryHint` as the direct
API.

`wasmModules` remains available for explicit injection when a runtime needs
static module bindings, as in the Edge example above. The previously tested
Chrome injection setup is a separate loading mode.

The MVP supports JPEG, PNG, and WebP input; JPEG and WebP output; metadata
stripping by default; best-effort and strict target-byte policies; and metrics
compatible with the native lazy-image vocabulary where possible. Resize policies
support `inside`, `cover`, and `fill`; `limits.timeoutMs` is enforced between
input, decode, resize, and encode stages as a best-effort checkpoint.
