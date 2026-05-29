# Benchmark Results Log

This file stores reproducible benchmark snapshots for new benchmark suites.

## Issue #377 (AVIF/Memory/Cold Start)

Date: 2026-02-16  
Branch: `codex/feature-377-bench-suite`  
Environment: local machine (Node.js + sharp installed)

### Commands

```bash
npm run test:bench:avif
npm run test:bench:memory
npm run test:bench:cold-start
```

### Results

#### AVIF / WebP / JPEG Matrix

| Case | lazy-image avg ms | sharp avg ms | speed ratio (sharp/lazy) | lazy size | sharp size | size diff | Note |
|---|---:|---:|---:|---:|---:|---:|---|
| AVIF q45 | 408.6 | 161.6 | 0.40x | 16.2 KB | 13.6 KB | 18.5% | lower quality / smaller output |
| AVIF q60 | 535.4 | 167.3 | 0.31x | 28.0 KB | 21.7 KB | 29.0% | default quality baseline |
| AVIF q75 | 652.4 | 202.2 | 0.31x | 39.9 KB | 34.1 KB | 17.0% | higher quality / larger output |
| WebP q80 | 178.2 | 79.3 | 0.45x | 31.7 KB | 29.0 KB | 9.3% | WebP baseline |
| JPEG q80 | 69.5 | 71.2 | 1.02x | 30.8 KB | 38.5 KB | -20.0% | JPEG baseline |

#### Memory Tracking (RSS Delta vs Estimate)

| Scenario | Estimated weight (MB) | Avg RSS delta (MB) | Avg metrics.peakRss (MB) | Avg time (ms) | Avg output (KB) | delta/estimate |
|---|---:|---:|---:|---:|---:|---:|
| jpeg-noops-q80 | 113.2 | 7.2 | 326.4 | 674.8 | 1196.2 | 0.06 |
| webp-resize800-q80 | 103.4 | 5.5 | 377.4 | 171.3 | 31.7 | 0.05 |
| jpeg-resize-rotate-gray-q75 | 103.4 | 7.5 | 430.3 | 105.5 | 47.6 | 0.07 |

#### Cold Start (Serverless-Oriented)

| Metric | Avg | p50 | p95 |
|---|---:|---:|---:|
| require() time | 2.9ms | 2.8ms | 4.5ms |
| first toBuffer() time | 30.5ms | 29.9ms | 35.7ms |
| total cold start | 33.5ms | 33.0ms | 40.2ms |

Notes:
- AVIF rows compare quality bands (q45/q60/q75) to expose practical speed/size trade-offs.
- `metrics.peakRss` is process peak RSS (`getrusage`) and not per-operation isolated memory.
- RSS delta is measured as before/after process RSS approximation, mainly for trend monitoring.

## Issue #619 (Benchmark Claim Refresh)

Date: 2026-05-29
Branch: `fix/benchmark-claims-619`
Environment: macOS 26.3, Apple M4 (arm64), Node.js v24.2.0, sharp v0.34.5, lazy-image v0.15.0

### Commands

```bash
npm run test:bench
node --expose-gc test/benchmarks/convert-only.bench.js
npm run test:bench:extended
```

### Canonical No-Resize Conversion

| Conversion | lazy-image | sharp | Speed Ratio (sharp/lazy) | Size Diff |
|------------|-----------:|------:|--------------------------:|----------:|
| PNG -> WebP | 4,323ms / 1,150,954 bytes | 909ms / 1,124,232 bytes | 0.21x | +2.4% |
| PNG -> AVIF | 12,668ms / 1,718,430 bytes | 4,729ms / 1,290,501 bytes | 0.37x | +33.2% |
| PNG -> JPEG | 700ms / 1,224,894 bytes | 660ms / 1,475,223 bytes | 0.94x | -17.0% |

### Resize 800px Comparison

| Scenario | lazy-image | sharp | Speed Ratio (sharp/lazy) | Size Diff |
|----------|-----------:|------:|--------------------------:|----------:|
| PNG -> JPEG resize 800px | 70ms / 31,518 bytes | 79ms / 39,416 bytes | 1.13x | -20.0% |
| PNG -> WebP resize 800px | 166ms / 32,448 bytes | 90ms / 29,686 bytes | 0.54x | +9.3% |
| PNG -> AVIF resize 800px | 442ms / 28,678 bytes | 179ms / 22,227 bytes | 0.40x | +29.0% |
| Resize + rotate + grayscale -> JPEG | 68ms / 27,129 bytes | 112ms / 24,650 bytes | 1.65x | +10.1% |

### Extended AVIF / WebP / JPEG Matrix

| Case | lazy-image avg ms | sharp avg ms | speed ratio (sharp/lazy) | lazy size | sharp size | size diff | Note |
|---|---:|---:|---:|---:|---:|---:|---|
| AVIF q45 | 331.8 | 157.6 | 0.48x | 16.2 KB | 13.6 KB | 18.5% | lower quality / smaller output |
| AVIF q60 | 449.1 | 164.2 | 0.37x | 28.0 KB | 21.7 KB | 29.0% | default quality baseline |
| AVIF q75 | 526.3 | 200.4 | 0.38x | 39.9 KB | 34.1 KB | 17.0% | higher quality / larger output |
| WebP q80 | 161.6 | 77.7 | 0.48x | 31.7 KB | 29.0 KB | 9.3% | WebP baseline |
| JPEG q80 | 57.4 | 71.8 | 1.25x | 30.8 KB | 38.5 KB | -20.0% | JPEG baseline |

### Memory Tracking

| Scenario | Estimated weight (MB) | Avg RSS delta (MB) | Avg metrics.peakRss (MB) | Avg time (ms) | Avg output (KB) | delta/estimate |
|---|---:|---:|---:|---:|---:|---:|
| jpeg-noops-q80 | 113.2 | 6.9 | 326.5 | 653.5 | 1196.2 | 0.06 |
| webp-resize800-q80 | 103.4 | 11.4 | 410.8 | 162.0 | 31.7 | 0.11 |
| jpeg-resize-rotate-gray-q75 | 103.4 | 9.0 | 484.8 | 90.5 | 47.6 | 0.09 |

### Cold Start

| Metric | Avg | p50 | p95 |
|---|---:|---:|---:|
| require() time | 3.0ms | 3.0ms | 3.5ms |
| first toBuffer() time | 28.5ms | 28.4ms | 29.5ms |
| total cold start | 31.5ms | 31.3ms | 32.5ms |

Notes:
- Current public size win is JPEG-specific in these measured scenarios.
- Current AVIF/WebP measurements favor sharp for speed and usually output size.
- Speed ratios use `sharp time / lazy-image time`; values below 1.0 mean sharp was faster.
