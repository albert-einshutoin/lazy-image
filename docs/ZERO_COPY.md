# Zero-Copy definition and validation

This document clarifies the **meaning, scope, and measurement** of lazy-image's "zero-copy" claims.

> "Zero-copy" in lazy-image refers to the **JS-heap boundary**, not the OS page cache. The source file is never staged through V8 heap memory. Whether the file is then accessed via a Rust-owned in-memory buffer or via `mmap` depends on its size — see "Implementation" below.

## Meaning

- **Zero-copy (JS-heap sense)**: when using `fromPath()` or `processBatch()` to `toFile()/toBuffer()`, the source file is **not copied into the Node.js (V8) heap**. JS never holds the raw file bytes.
- **Implementation** (size-tiered to eliminate SIGBUS/SIGSEGV risk for typical images):
  - **Files ≤ 256 MB**: read in full into a Rust-owned buffer. No mmap. No V8 heap copy. (`MMAP_SIZE_THRESHOLD` in [`src/engine/io.rs`](../src/engine/io.rs).)
  - **Files > 256 MB**: opened via `mmap` with an advisory lock and an fd retained for the engine lifetime. Page-cache backed; still no V8 heap copy.
- In both tiers, decoded pixel buffers live in Rust memory.

## Scope (where it applies)

Applies:
- `ImageEngine.fromPath(...)` → any processing → `toFile()/toBuffer()/toBufferWithMetrics()` (no V8-heap copy of the source bytes)
- `processBatch()` (per-input, same size-tiered model)
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
  「ゼロコピー」の主張は「入力を V8 (JS) ヒープに載せない」ことを指すため、ヒープ増加が事実上の証拠となる。
- **出力バッファはコピーになるのでは?**  
  はい。エンコード結果は必ず `Buffer` として生成されるため、出力サイズ分のメモリは必要。ゼロコピーの対象は「入力経路」である。
- **mmap は常に使われるのか?**  
  いいえ。SIGBUS/SIGSEGV を避けるため、256 MB 以下のファイルは Rust 側で全読み込みする。256 MB を超えるファイルだけが advisory lock 付きの mmap を使う。`MMAP_SIZE_THRESHOLD` を参照。
- **ストリーミング API は?**  
  デフォルトはディスクバッファを使うが、入力ストリームを JS で保持する場合はゼロコピーの対象外。ただし内部処理は同じメモリモデルを使う。

## まとめ

- **ゼロコピー = 入力ファイルを V8 ヒープへコピーしない**（256 MB 以下は Rust 所有メモリ、256 MB 超は mmap）
- **測定式**で上限を示し、`docs/scripts/measure-zero-copy.js` でいつでも再検証できる
- 適用範囲と例外を明示し、期待値と境界をドキュメント化

## Behavior when files are modified or deleted during mmap

> This section applies only to the > 256 MB mmap tier. Files ≤ 256 MB are read into a Rust-owned buffer up front and are no longer touched on disk after that read completes, so concurrent mutation does not affect them.

- **Contract**: Source files larger than 256 MB must not be modified or deleted while processing is in progress.
- **Possible outcomes on Linux/macOS**: decode failure, corrupted images, or a fatal `SIGBUS`/`SIGSEGV` if mapped pages become invalid (for example due to truncation).
- **Possible outcomes on Windows**: deletion typically fails while the mapping is active; concurrent modification can still cause decode failure or corrupted output.
- **Recommendations**:
  - If files may change during processing, use copy paths such as `from(Buffer)` or copy to a temporary local path first.
  - To prevent concurrent writes on shared storage, use file locking (for example, `flock`-equivalent OS locks).
  - On Windows, keep source files until processing completes, or switch to `from(Buffer)` when immediate deletion is required.

## Additional mmap safety assumptions (fromPath/processBatch, > 256 MB tier)

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

Windows host memory detection is supported for auto concurrency. `processBatch()` results also include the effective worker count so callers can verify what auto mode selected in production.

### Serverless Cold Start

First invocation has additional overhead:
- jemalloc initialization (~1-2ms)
- rayon thread pool creation via `OnceLock` (~1-3ms)
- First codec initialization (format-dependent)

Subsequent invocations reuse the thread pool and have no initialization overhead.
