# Thread Model

This document describes how lazy-image uses threads and interacts with Node.js and Rust thread pools.

## Overview

lazy-image uses two thread pools:

1. **libuv thread pool** (Node.js) - for I/O operations
2. **rayon thread pool** (Rust) - for CPU-bound image processing
   - **fast_image_resize also uses rayon** when its `rayon` feature is enabled (default in this repo). When resize runs inside `processBatch()` we wrap work in our global pool, so fast_image_resize reuses that pool (no duplicate thread pools). Single `toBuffer()` calls run on libuv worker threads, so they only consume the global rayon pool (default configuration), making it difficult to oversubscribe threads.

```
┌─────────────────────────────────────────────────────────────┐
│                    Node.js Main Thread                       │
│                  (Event loop, JavaScript)                    │
├─────────────────────────────────────────────────────────────┤
│                     NAPI AsyncTask                           │
│            (Bridges JS async to Rust threads)                │
├─────────────────────────────────────────────────────────────┤
│   libuv Thread Pool        │      rayon Thread Pool         │
│   (UV_THREADPOOL_SIZE)     │      (available_parallelism)   │
│   - File I/O               │      - Batch processing        │
│   - DNS lookups            │      - Parallel image work     │
│   - crypto                 │                                │
└─────────────────────────────────────────────────────────────┘
```

## Thread Usage by API

| Method | Thread Pool | Notes |
|--------|-------------|-------|
| `toBuffer()` | libuv | Single image processing |
| `toBufferWithMetrics()` | libuv | Single image with metrics |
| `toFile()` | libuv | Single image to file |
| `processBatch()` | **rayon** | Parallel batch processing |

## Concurrency Control

### Single Image Operations

For `toBuffer()`, `toFile()`, etc., each call runs on a libuv worker thread.

**Default libuv pool size**: 4 threads

To increase (set before starting Node.js):
```bash
export UV_THREADPOOL_SIZE=16
node your-app.js
```

### Batch Processing

`processBatch()` uses rayon for parallel execution:

```javascript
// Default: automatically calculates safe thread count
// Prevents oversubscription by reserving threads for libuv
const results = await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
});

// Limit concurrency explicitly (memory-constrained environments)
const results = await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
  concurrency: 4,
});
```

- **Concurrency parameter**:
- `0` or `undefined`: Automatically detects a safe thread count using
  `std::thread::available_parallelism()` (respects cgroup / CPU quotas), then
  subtracts `UV_THREADPOOL_SIZE` (default 4) so rayon + libuv stay within the
  quota. Detection always leaves at least one rayon worker.
- `1-1024`: Use specified number of workers (manual control)

## Memory Considerations

### Per-Image Memory

Each image being processed requires approximately:

```
Memory = width × height × 4 bytes (RGBA)
```

Examples:
- 1920×1080 (Full HD): ~8 MB
- 3840×2160 (4K): ~33 MB
- 6000×4000 (24MP): ~96 MB

### Batch Processing Memory

When using `processBatch()`:

```
Peak Memory ≈ single_image_memory × concurrency
```

**Example**: Processing 24MP images with concurrency=4:
```
Peak Memory ≈ 96 MB × 4 = ~400 MB
```

### Recommendations

| Environment | Recommendation |
|-------------|----------------|
| 512MB container | `concurrency: 2` for large images |
| 1GB container | `concurrency: 4` recommended |
| 4GB+ container | Default (use all cores) is safe |

## Docker / Kubernetes

### CPU Limits

In containerized environments, Node.js and Rust may see different CPU counts:

- **Node.js** (via `os.cpus().length`): May see host CPUs, not container limit
- **rayon** (via `std::thread::available_parallelism()`): ✅ Respects cgroup/CPU quota automatically

**Note**: While `processBatch()` with default concurrency (`concurrency=0`) automatically respects cgroup limits, you can still specify concurrency explicitly for fine-grained control:

```javascript
// In Docker/Kubernetes, don't rely on defaults
const CONCURRENCY = parseInt(process.env.CONCURRENCY || '4');
await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
  concurrency: CONCURRENCY,
});
```

### Docker Resource Configuration

```yaml
# docker-compose.yml
services:
  image-processor:
    image: your-app
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 2G
    environment:
      - CONCURRENCY=4
```

### Kubernetes Resource Configuration

```yaml
# deployment.yaml
spec:
  containers:
  - name: image-processor
    resources:
      limits:
        cpu: "4"
        memory: "2Gi"
    env:
    - name: CONCURRENCY
      value: "4"
```

## Best Practices

### 1. Use File-Based I/O for Large Images

```javascript
// ✅ Good: Bypasses V8 heap
await ImageEngine.fromPath('large.tiff')
  .resize(800)
  .toFile('output.jpg', 'jpeg', 80);

// ❌ Bad: Loads entire file into V8 heap
const buffer = fs.readFileSync('large.tiff'); // OOM risk
await ImageEngine.from(buffer).toBuffer('jpeg', 80);
```

### 2. Control Batch Concurrency

```javascript
// ✅ Good: Explicit concurrency control (recommended for production)
await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
  concurrency: 4,
});

// ✅ Also Good: Default now automatically balances threads
// Safe for most use cases, but explicit is better in containers
await engine.processBatch(manyLargeFiles, outDir, {
  format: 'webp',
  quality: 80,
});
```

### 3. Monitor Memory in Production

```javascript
const v8 = require('v8');

function logMemory() {
  const stats = v8.getHeapStatistics();
  console.log({
    heapUsed: Math.round(stats.used_heap_size / 1024 / 1024) + 'MB',
    heapTotal: Math.round(stats.total_heap_size / 1024 / 1024) + 'MB',
  });
}
```

## Thread Pool Coordination

### Automatic Thread Pool Balancing

To prevent resource exhaustion and server crashes, `processBatch()` automatically
detects a safe thread count when `concurrency=0` (default):

1. **Queries `std::thread::available_parallelism()`** – respects cgroup/CPU quota
2. **Reserves `UV_THREADPOOL_SIZE` threads** (default 4) for libuv, subtracting
   that from the rayon pool size. This keeps total active threads within quota.
3. **Falls back** to at least one rayon worker if detection fails or the reserve
   would drop the pool to zero.

**Example on 8-core host limited to 6 CPUs via cgroup**:
- `available_parallelism()` returns 6
- Reserve: `UV_THREADPOOL_SIZE=4`
- Rayon threads: `max(1, 6 - 4) = 2` → 4 libuv + 2 rayon = 6 total ✅

**Example on tiny container (quota = 1 CPU)**:
- `available_parallelism()` returns 1
- Reserve 4 threads but clamp result to minimum 1
- Rayon threads: 1 (minimum) → libuv threads may oversubscribe slightly but IO
  rarely saturates CPU in such constrained environments

### Thread Pool Lifecycle

- The rayon pool is created lazily on the first batch workload and reused for the process lifetime.
- Embedding or test harnesses that reload the addon can drop the pool between runs via the internal
  `shutdown_global_pool()` hook (Rust-only) so that updated `UV_THREADPOOL_SIZE` or resource limits
  are honored when the pool is rebuilt.

### Manual Thread Pool Control

For fine-grained control, set environment variables:

```bash
# Set libuv pool size (must be set before Node.js starts)
export UV_THREADPOOL_SIZE=8

# Explicitly set concurrency in code
await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
  concurrency: 4,
});
```

**Best Practice**: The default behavior (`concurrency=0`) automatically respects cgroup/CPU quotas, making it safe for most containerized environments. However, for fine-grained control or memory-constrained environments, explicitly setting `concurrency` is still recommended.

## Known Issues

### Dual Thread Pool Interaction (Resolved)

~~When using both libuv and rayon threads heavily, scheduling may become unpredictable.~~

**Status**: ✅ **Fixed in v0.7.8+**

The default behavior of `processBatch()` now automatically balances thread pools to prevent oversubscription. Manual coordination is no longer required for most use cases.

**Remaining considerations**:
- For batch processing, prefer `processBatch()` (pure rayon) over parallel `Promise.all()` with many `toBuffer()` calls
- In containerized environments, still recommend explicit `concurrency` parameter
- fast_image_resize's internal parallelism uses the "currently active rayon pool". Since `processBatch()` executes resize within the global pool, the pools are shared. If you call `rayon::ThreadPoolBuilder::new().build_global()` separately, unify the pools to prevent oversubscription from duplicate pools.

## References

- [libuv Thread Pool](https://docs.libuv.org/en/v1.x/threadpool.html)
- [rayon Documentation](https://docs.rs/rayon/latest/rayon/)
- [NAPI-RS AsyncTask](https://napi.rs/docs/concepts/async-task)
