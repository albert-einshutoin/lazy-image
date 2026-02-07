# Troubleshooting

Common issues and how to fix them. See also [ERROR_CODES.md](./ERROR_CODES.md) and [INPUT_VALIDATION.md](./INPUT_VALIDATION.md).

## Error recovery

- **E1xx (input)** — Check path, permissions, format support, and dimension limits.
- **E2xx (processing)** — Adjust crop bounds, rotation (90/180/270 only), or other ops.
- **E3xx (output)** — Check disk space, path writable, encode format support.
- **E4xx (config)** — Fix preset name, options, or firewall limits.

Use `error.recoveryHint` when present. Recoverable errors (path, bounds, etc.) can be fixed and retried; non-recoverable (unsupported format, encode failure) require different input or handling.

## Windows file locking

**Symptom**: Deleting or overwriting the source file after `fromPath()` fails on Windows.

**Cause**: Windows does not allow deleting a file while it is memory-mapped.

**Fix**:

- Keep the engine in a small scope so it is dropped before you delete the file:
  ```javascript
  {
    const engine = ImageEngine.fromPath('input.jpg');
    await engine.resize(800).toFile('output.jpg', 'jpeg', 80);
  }
  fs.unlinkSync('input.jpg');
  ```
- Or use the heap path: `ImageEngine.from(fs.readFileSync('input.jpg'))` if you must delete immediately (uses more memory).
- For `processBatch()`, do not delete input files until the batch has finished.

## Streaming (disk-backed pipeline)

`createStreamingPipeline()` gives **bounded-memory**, disk-backed processing: it stages input to a temp file, runs `ImageEngine.fromPath` on it, and streams the encoded result from another temp file. It is **not** real-time chunked encoding; latency includes writing to disk first. For true streaming transforms, use another solution or a future dedicated API.

## Dimension / size limits

- Default max dimension: 32768 px (decompression bomb protection).
- Image Firewall: `.sanitize({ policy: 'strict' })` caps at 40 MP and 32 MB input; `'lenient'` at 75 MP and 48 MB. Use `.limits({ maxPixels, maxBytes, timeoutMs })` to override. See [ARCHITECTURE.md](./ARCHITECTURE.md#image-firewall-mode-strict--lenient).

## Build / install issues

- **Native build fails**: Ensure Node.js 18+, Rust 1.70+, and (for mozjpeg SIMD) nasm. For libavif, cmake is required.
- **Platform binary missing**: Use `npm run build` to build from source, or check [GitHub Actions](https://github.com/albert-einshutoin/lazy-image/actions) that the release for your platform was published.

## mmap and file modification

Do not modify, truncate, or delete the source file while it is being read via `fromPath()` or inside `processBatch()`. Results are undefined (decode errors, corruption). On Windows, deletion will fail. Use a copy or the buffer API if the file must change during processing.
