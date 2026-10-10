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

## Build a static page from the manifest

The [static-site example](../examples/compile-image-static-site.mjs) compiles one
local image, then builds a page from the returned manifest. It uses the same
public API; it does not implement another image verifier or staging transaction.
Use Node.js 22 or newer on a [supported native platform](./COMPATIBILITY.md).

From a checkout with a current native build:

```bash
mkdir -p public
node examples/compile-image-static-site.mjs \
  test/fixtures/test_100KB_1057x1057.jpg public/photo-v1
node examples/compile-image-static-site.mjs --self-test
```

The default policy requests widths `[320, 640, 960]`, WebP, and a placeholder.
To supply another compiler policy, pass a JSON file as the third argument:

```json
{"widths":[320,640],"formats":["webp"],"placeholder":true}
```

```bash
node examples/compile-image-static-site.mjs input.jpg public/photo-v2 policy.json
```

The output is `index.html` beside `images/manifest.json`, the compiler-owned image
files, and the optional placeholder. The page maps each manifest-relative path
under `images/`; its `srcset`, image dimensions and placeholder URL come from the
actual manifest. Requested widths are not filenames. A 64px-wide input produces
a 64px variant with this default policy, rather than links to absent 320/640/960px
images. The script does not put input/output filesystem paths in HTML.
For multiple formats it orders picture sources AVIF, then WebP, with JPEG as the
compatible `img` fallback when present; source preference is not a size/quality claim.

Serve **only the generated site directory**, for example with Python's standard
static server (Python 3 is needed only for this preview command):

```bash
python3 -m http.server 8080 --bind 127.0.0.1 --directory public/photo-v1
# Open http://127.0.0.1:8080/ and inspect the image and Compiler manifest link.
```

This relative URL layout is application code. Your site build and hosting own URL
placement and delivery. The compiler commits only the `images/` set; HTML is
written afterwards. If HTML writing fails, verified images may remain without a
page. This example is not a transaction for an entire site or a multi-image job.
Use an existing trusted parent, do not replace it concurrently, and always choose
a **new** site directory. Existing directories are rejected before any write to
their contents; the example never deletes or overwrites them.

Success exits `0`; argument/policy, compilation or HTML errors exit `1` and report
the phase/code, recovery hint when available, and any attached cleanup error on
stderr. Invalid input and an unmet strict budget produce no success page. A
failure can leave the new empty site directory, or verified images if page writing
failed; inspect the diagnostic and use a new destination after correcting the
input/policy/permissions. Do not remove a prior site simply to retry. The
`--self-test` checks command exits, manifest/files/page references, no-upscale,
invalid input, strict budget failure, and byte-for-byte preservation on a retry
against an existing site. Browser decoding/display is a separate preview check.

To try the same example against the fixed **published** package outside a checkout,
copy `examples/compile-image-static-site.mjs` and `examples/_load.mjs` into a new
directory, then:

```bash
npm init -y
npm install --save-exact --registry=https://registry.npmjs.org @alberteinshutoin/lazy-image@1.4.3
node compile-image-static-site.mjs /path/to/input.jpg ./site-v1
python3 -m http.server 8080 --bind 127.0.0.1 --directory ./site-v1
```

`--self-test` uses repository fixtures and is intended for the checkout. The
[recorded integration trial](./history/STATIC_SITE_ADOPTION.md) includes published
installation provenance, commands, failures and browser evidence. It is a
technical reproduction, not an independent user's adoption or performance claim.
The [existing build-time batch example](../examples/build-time-optimize.mjs)
optimizes several files using `ImageEngine` and reports metrics; it does not
provide the compiler manifest-to-page flow demonstrated here.

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
- [Compiler static page](../examples/compile-image-static-site.mjs): one image's verified manifest consumed by HTML.
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
