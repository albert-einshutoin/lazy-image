# Examples

Runnable examples demonstrating lazy-image usage patterns.

## Prerequisites

```bash
npm install @alberteinshutoin/lazy-image
# or build from source:
git clone https://github.com/albert-einshutoin/lazy-image.git
cd lazy-image && npm install && npm run build
```

## Examples

| File | What it demonstrates |
|------|----------------------|
| [basic.mjs](./basic.mjs) | Resize, convert, crop, rotate, presets, and metrics |
| [batch-processing.mjs](./batch-processing.mjs) | Batch processing with auto/manual concurrency |
| [error-handling.mjs](./error-handling.mjs) | 4-tier error taxonomy, validation, and Image Firewall handling |
| [upload-sanitize-server.mjs](./upload-sanitize-server.mjs) | Raw upload endpoint with header inspection, Image Firewall, and HTTP status mapping |
| [build-time-optimize.mjs](./build-time-optimize.mjs) | Static asset batch optimization with metrics summary and failure reporting |
| [metrics-observability.mjs](./metrics-observability.mjs) | Structured JSON logs and aggregate counters from `ProcessingMetrics` |

## Running

```bash
node examples/basic.mjs
node examples/batch-processing.mjs
node examples/error-handling.mjs
node examples/upload-sanitize-server.mjs
node examples/build-time-optimize.mjs ./public/images ./public/optimized
node examples/metrics-observability.mjs ./input.jpg
```

Each example also has a fixture-backed self-test mode used by CI:

```bash
npm run test:examples
node examples/upload-sanitize-server.mjs --self-test
```

These examples use ESM (`import`). For CommonJS, replace `import` with `require()`.
