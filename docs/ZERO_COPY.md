# Zero-Copy definition and validation

This document clarifies the **meaning, scope, and measurement** of lazy-image's zero-copy claims.

## Meaning

- **Zero-copy**: when using `fromPath()` or `processBatch()` to `toFile()/toBuffer()`, the source file is **not copied into the Node.js heap**.
- Implementation: input files are accessed via **mmap**, read directly by Rust; JS never holds the raw file bytes.

## Scope (where it applies)

Applies:
- `ImageEngine.fromPath(...)` → any processing → `toFile()/toBuffer()/toBufferWithMetrics()`
- `processBatch()` (each input handled via mmap)
- Rust-side decode/encode (pixel buffers live in Rust memory)

Not applicable / exceptions:
- `ImageEngine.from(Buffer)` / `fromBytes()` / `fromMemory()` where JS already owns the buffer.
- Explicit conversions to `Vec<u8>` (e.g., `as_vec()`), which break zero-copy.
- Output buffers (`toBuffer*`) are always copied (expected).
- Windows: files cannot be deleted while mmap is active; manage lifecycle accordingly.

## Measurable targets

1. **JS heap growth**: `fromPath → toBufferWithMetrics` should increase `heapUsed` by ≤ **2 MB** (measure with `node --expose-gc docs/scripts/measure-zero-copy.js`).
2. **RSS budget**: `peak_rss ≤ decoded_bytes + 24 MB`  
   - `decoded_bytes = width × height × bpp` (`bpp`: JPEG=3, PNG/WebP/AVIF=4)  
   - 24 MB is the safety margin for decode/encode working buffers and threads.
3. **Example**: 6000×4000 PNG (24 MP, bpp=4) → `decoded_bytes ≈ 96 MB`, target `peak_rss ≤ 120 MB`.

These numbers are reproducible via the measurement script; open an issue/PR if you observe deviations.

## Measurement steps

1. Run Node in GC-enabled mode: `node --expose-gc docs/scripts/measure-zero-copy.js`
2. Example JSON output:
   ```json
   {
     "source": "test_4.5MB_5000x5000.png",
     "rss_start_mb": 30.1,
     "rss_end_mb": 118.4,
     "rss_delta_mb": 88.3,
     "heap_delta_mb": 0.7,
     "peak_rss_metrics_mb": 116.9
   }
   ```
3. Pass criteria:
   - `heap_delta_mb <= 2.0`
   - `rss_end_mb` within 10% of the budget formula (e.g., budget 120 MB → limit ≈ 132 MB)

## FAQ

- **なぜ JS ヒープを指標にするのか?**  
  ゼロコピーの主張は「入力を JS ヒープに載せない」ことにあるため、ヒープ増加が事実上の証拠となる。
- **出力バッファはコピーになるのでは?**  
  はい。エンコード結果は必ず `Buffer` として生成されるため、出力サイズ分のメモリは必要。ゼロコピーの対象は「入力経路」である。
- **ストリーミング API は?**  
  デフォルトはディスクバッファを使うが、入力ストリームを JS で保持する場合はゼロコピーの対象外。ただし内部処理は同じメモリモデルを使う。

## まとめ

- **ゼロコピー = 入力ファイルを JS ヒープへコピーしない**（mmap で Rust から直接読む）
- **測定式**で上限を示し、`docs/scripts/measure-zero-copy.js` でいつでも再検証できる
- 適用範囲と例外を明示し、期待値と境界をドキュメント化

## Behavior when files are modified or deleted during mmap

- **Contract**: Source files must not be modified or deleted while processing is in progress.
- **Possible outcomes on Linux/macOS**: decode failure, corrupted images, or a fatal `SIGBUS`/`SIGSEGV` if mapped pages become invalid (for example due to truncation).
- **Possible outcomes on Windows**: deletion typically fails while the mapping is active; concurrent modification can still cause decode failure or corrupted output.
- **Recommendations**:
  - If files may change during processing, use copy paths such as `from(Buffer)` or copy to a temporary local path first.
  - To prevent concurrent writes on shared storage, use file locking (for example, `flock`-equivalent OS locks).
  - On Windows, keep source files until processing completes, or switch to `from(Buffer)` when immediate deletion is required.

## Additional mmap safety assumptions (fromPath/processBatch)

- Files must stay readable for the engine lifetime; permission changes or truncation after mmap are undefined behavior.
- File size is assumed stable: truncation/extension after mmap may SIGBUS or corrupt output.
- Network/distributed filesystems (NFS/SMB/etc.) can propagate remote edits; prefer local/temp copies when consistency is required.
- Delete-after-write: copy to a temp path or use `from(Buffer)`; mmap keeps the file open until the engine (and its clones) are dropped.
- For transactional reads, take an advisory lock or use an immutable snapshot/copy before calling `fromPath`.

## Copy-on-Write points (deep-copy triggers)

- `apply_ops_tracked`: `Cow<DynamicImage>` materializes via `into_owned()` when any op exists (format-only path stays borrowed).
- Resize paths normalize to RGBA via `to_rgba8()` if not already RGB/RGBA.
- Encoders allocate output buffers: `toBuffer*` returns a fresh Node.js `Buffer`; `toFile` writes encoded data to disk.
- Debugging: enable feature `cow-debug` and set `LAZY_IMAGE_DEBUG_COW=1` to emit `tracing::debug!` logs for each copy point.

### How to avoid extra copies
- Keep inputs in RGB/RGBA when possible to skip `to_rgba8()`.
- Chain multiple outputs via `clone()` before applying divergent ops to reuse decoded data.
- Prefer `fromPath()` over `from(Buffer)` for large files to avoid JS-heap copies.

### Windows-specific safe usage patterns

- **Immediate deletion**: 
  ```js
  const buf = fs.readFileSync(src); // JS heap path
  const out = await ImageEngine.from(buf).toFile(dst, 'jpeg', 80);
  fs.unlinkSync(src); // OK
  ```
- **Copy to temporary directory for processing**:
  ```js
  const tmp = path.join(os.tmpdir(), path.basename(src));
  fs.copyFileSync(src, tmp);
  await ImageEngine.fromPath(tmp).toFile(dst, 'jpeg', 80);
  fs.unlinkSync(tmp); // Original file remains unchanged
  ```
- **Batch processing**: Keep input files during `processBatch()` execution and delete after completion (after confirming that the scope has exited and mmap is closed).

## Caveats and Platform-Specific Behavior

For mmap safety requirements and failure modes, see:
- `Behavior when files are modified or deleted during mmap`
- `Additional mmap safety assumptions (fromPath/processBatch)`

### Windows Memory Detection

`detect_system_memory()` returns `None` on Windows. Batch processing falls back to CPU-only concurrency estimation, which may cause OOM with many concurrent large images. Consider setting explicit concurrency limits on Windows.

### Serverless Cold Start

First invocation has additional overhead:
- jemalloc initialization (~1-2ms)
- rayon thread pool creation via `OnceLock` (~1-3ms)
- First codec initialization (format-dependent)

Subsequent invocations reuse the thread pool and have no initialization overhead.
