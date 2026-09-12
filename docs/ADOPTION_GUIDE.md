# Adoption guide

Choose a complete artifact set for uploads/builds, or an individual output when
that is all your application needs. Install the [Node package](../README.md#install)
first. Code examples below use ESM and top-level `await` in a `.mjs` file.

## Start with a public artifact set

Create a new destination beneath an existing, trusted parent directory:

```javascript
import { compileImage } from '@alberteinshutoin/lazy-image';

const manifest = await compileImage({
  inputPath: './uploads/photo.jpg',
  outputDir: './public/images/photo-v1',
  policy: { widths: [320, 640], formats: ['webp'], placeholder: true },
});
console.log(manifest);
```

For a source at least 640 pixels wide, this produces:

```text
photo-v1/
  image-320.webp
  image-640.webp
  placeholder.webp
  manifest.json
```

The compiler does not enlarge smaller sources. It normalizes the requested widths;
read the actual artifacts from the manifest rather than assuming every requested
width was produced. See [compiler policy](./API.md#compiler-policy) for defaults,
byte budgets and input restrictions.

### Use the CLI

Save this as `policy.json`:

```json
{"widths":[320,640],"formats":["webp"],"placeholder":true}
```

```bash
npx @alberteinshutoin/lazy-image compile uploads/photo.jpg \
  --out-dir public/images/photo-v1 --policy policy.json
```

The CLI uses the same compiler contract. Success writes manifest JSON to stdout;
diagnostics go to stderr. Exit codes: `0` success, `2` invalid arguments or policy
JSON, `1` compiler failure. Use a new output directory for each compilation.

### Publish through your application

The compiler checks files in private staging, then commits the directory locally.
The parent must be trusted and must not be replaced concurrently; staging and
output must share a filesystem. Source-snapshot cleanup completes before commit.
A failed compilation does not publish the set. In the JavaScript API, an additional
cleanup failure is attached to the rejected error as `error.cleanupError`; inspect
it along with the primary phase/code. The CLI prints only the primary failure and
does not print the attached cleanup error. CLI stderr alone therefore cannot
confirm that unpublished staging was removed. Do not delete an existing
destination simply to retry.

Your application owns authentication, upload admission, job scheduling, storage
transfer and delivery. Upload the verified files through your existing storage
integration, then make their URLs available to consumers. That remote publication
is separate from the compiler's local transaction. Map manifest-relative paths to
a public prefix; never send private filesystem paths to a browser.

## Optimize one image

Use the asynchronous file constructor in servers so source setup runs off the
Node.js event loop:

```javascript
import { ImageEngine } from '@alberteinshutoin/lazy-image';

const image = await ImageEngine.fromPathAsync('./uploads/photo.jpg');
await image
  .sanitize({ policy: 'public-upload' })
  .resize({ width: 1600, height: 1600, fit: 'inside' })
  .toFile('./public/photo.webp', 'webp', 80);
```

`public-upload` strips EXIF/GPS/XMP and keeps only validated ICC within the policy
limits. It is not the same as publishing a verified multi-file set.
See [metadata behavior](./METADATA_SUPPORT.md).

| Input/output choice | Use when | Cost or constraint |
|---|---|---|
| `fromPathAsync()` → `toFile()` | Input and output are files | Source bytes stay Rust-owned; decoded pixels still use native memory |
| `from(buffer)` → `toBuffer()` | The caller needs in-memory bytes | Input is copied into Rust; encoded output becomes a Node Buffer |
| `createStreamingPipeline()` | Existing integration requires streams | Stages input/output on disk; not incremental decode/encode |

Writing an already-buffered upload to disk does not undo its memory allocation.
Bound upload admission and concurrency. For large mutable sources, use a stable
copy; see [file memory and mmap rules](./ZERO_COPY.md).

## Build several outputs

Use `compileImage()` for a publication contract; use `clone()`, responsive helpers
or batch APIs when your application manages output names and partial failures.
The general responsive helpers can enlarge images and do not promise the compiler's
transaction. See [API](./API.md#responsive-output-helpers).

Runnable examples are maintained separately:

- [Build-time batch](../examples/build-time-optimize.mjs): metrics and failure reporting.
- [Upload endpoint](../examples/upload-sanitize-server.mjs): raw uploads and error mapping.
- [Observability](../examples/metrics-observability.mjs): structured metrics logs.

## Deploy and measure

Use the native package on its [supported Node platforms](./COMPATIBILITY.md).
V8 isolates require the separate [Wasm preflight package](./WASM_PACKAGE_API.md),
which does not expose the Node compiler. Match platform/architecture/libc to the
installed native package; verify installation in the actual deployment image.

Size memory, temporary disk, concurrency and timeouts from representative inputs.
Do not assume avoiding a V8 copy prevents OOM or that a smaller native binary
establishes lower cold-start latency. Use [metrics](./metrics-api.md),
[thread configuration](./THREAD_MODEL.md) and [performance evidence](./PERFORMANCE.md).
For failures, start with [troubleshooting](./TROUBLESHOOTING.md).
