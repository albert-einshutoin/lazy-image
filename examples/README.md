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

| File | Description |
|------|-------------|
| [basic.mjs](./basic.mjs) | Resize, convert, crop, rotate, presets, metrics |
| [batch-processing.mjs](./batch-processing.mjs) | Batch processing with auto/manual concurrency |
| [error-handling.mjs](./error-handling.mjs) | 4-tier error taxonomy, validation, Image Firewall |

## Running

```bash
node examples/basic.mjs
node examples/batch-processing.mjs
node examples/error-handling.mjs
```

These examples use ESM (`import`). For CommonJS, replace `import` with `require()`.
