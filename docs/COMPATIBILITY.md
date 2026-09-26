# Compatibility and limits

The native Node package and the Wasm preflight package have different APIs.
For product fit, see [product direction](./PROJECT_PHILOSOPHY.md).

## Runtime

The native package requires Node.js 22 or newer and provides platform packages for:

- macOS arm64 and x64
- Linux x64 GNU/musl and arm64 GNU
- Windows x64

Match the deployment OS, architecture and libc. V8 isolates do not load native
N-API binaries. The [Wasm package](./WASM_PACKAGE_API.md) has a narrower browser/Edge
preflight contract; its Node tooling requires Node.js 22 or newer.

## Formats and operations

| Surface | Input | Output |
|---|---|---|
| `ImageEngine` | JPEG, PNG, WebP | JPEG, PNG, WebP, AVIF |
| `compileImage()` / CLI | Static JPEG, PNG, WebP within compiler limits | JPEG, WebP, AVIF; optional WebP placeholder |
| Wasm preflight | See [Wasm API](./WASM_PACKAGE_API.md) | JPEG / WebP under its own policy |

AVIF output depends on the native build's `avif` feature. Query
`supportedInputFormats()` and `supportedOutputFormats()` for the loaded engine.
Compiler policy is narrower than the general engine: see [policy limits](./API.md#compiler-policy).

The native engine supports resize, crop, 90°/180°/270° rotation, flips, grayscale,
brightness and contrast. It does not provide compositing, rich filters, SVG/TIFF
input or animated-image editing. 16-bit inputs are converted to 8-bit.

## Behavioral boundaries

- Metadata is stripped by default; ICC/EXIF preservation is opt-in and format-dependent.
  GPS is stripped even when EXIF is retained unless explicitly allowed. See [metadata](./METADATA_SUPPORT.md).
- `fromPathAsync()` is the server entrypoint; `fromPath()` performs synchronous setup.
  Both use native memory. See [file I/O](./ZERO_COPY.md).
- The streaming helper stages to disk; it is not incremental decode/encode.
- General responsive helpers and batch outputs are not a transactional artifact set.
- lazy-image is not a drop-in sharp API. Check [migration differences](./MIGRATION_FROM_SHARP.md)
  before translating a pipeline; equal quality numbers do not imply equal output quality.
