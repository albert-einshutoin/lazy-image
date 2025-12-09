# Thread Model

This document describes how lazy-image uses threads and interacts with Node.js and Rust thread pools.

## Overview

lazy-image uses two thread pools:

1. **libuv thread pool** (Node.js) - for I/O operations
2. **rayon thread pool** (Rust) - for CPU-bound image processing

```
┌─────────────────────────────────────────────────────────────┐
│                    Node.js Main Thread                       │
│                  (Event loop, JavaScript)                    │
├─────────────────────────────────────────────────────────────┤
│                     NAPI AsyncTask                           │
│            (Bridges JS async to Rust threads)                │
├─────────────────────────────────────────────────────────────┤
│   libuv Thread Pool        │      rayon Thread Pool         │
│   (UV_THREADPOOL_SIZE)     │      (num_cpus::get())         │
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
// Default: uses all CPU cores
const results = await engine.processBatch(files, outDir, 'webp', 80);

// Limit concurrency (memory-constrained environments)
const results = await engine.processBatch(files, outDir, 'webp', 80, 4);
```

**Concurrency parameter**:
- `0` or `undefined`: Use all CPU cores (default)
- `1-1024`: Use specified number of workers

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
- **rayon** (via `num_cpus`): May see host CPUs, not container limit

**Solution**: Always specify concurrency explicitly in containers:

```javascript
// In Docker/Kubernetes, don't rely on defaults
const CONCURRENCY = parseInt(process.env.CONCURRENCY || '4');
await engine.processBatch(files, outDir, 'webp', 80, CONCURRENCY);
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
      - UV_THREADPOOL_SIZE=8
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
    - name: UV_THREADPOOL_SIZE
      value: "8"
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
// ✅ Good: Explicit concurrency control
await engine.processBatch(files, outDir, 'webp', 80, 4);

// ⚠️ Risky: Default may overwhelm memory
await engine.processBatch(manyLargeFiles, outDir, 'webp', 80);
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

## Known Issues

### Dual Thread Pool Interaction

When using both libuv and rayon threads heavily, scheduling may become unpredictable.

**Mitigation**:
- For batch processing, prefer `processBatch()` (pure rayon)
- Avoid parallel `Promise.all()` with many `toBuffer()` calls
- Use explicit concurrency limits

## References

- [libuv Thread Pool](https://docs.libuv.org/en/v1.x/threadpool.html)
- [rayon Documentation](https://docs.rs/rayon/latest/rayon/)
- [NAPI-RS AsyncTask](https://napi.rs/docs/concepts/async-task)
