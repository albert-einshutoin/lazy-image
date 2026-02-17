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
