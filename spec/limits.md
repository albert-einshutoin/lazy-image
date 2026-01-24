# Limits & Resource Contracts (v0.9.x)

## Dimensions & pixels
- `MAX_DIMENSION = 32,768` per side.
- `MAX_PIXELS = 100,000,000` per image (after resize/crop).
- Violations raise `ResourceLimit` errors before processing.

## Image Firewall (input safeguards)
Default policy: **strict** unless caller sets `sanitize({ policy: "lenient" })` or overrides via `limits()`.

| Policy   | maxBytes | maxPixels | timeoutMs | Notes |
|----------|----------|-----------|-----------|-------|
| strict   | 32 MB    | 100 MP    | 5,000 ms  | tuned for typical web inputs; blocks heavy AVIFs sooner |
| lenient  | 48 MB    | 100 MP    | 30,000 ms | allows larger/slow AVIF while keeping pixel cap |

- Callers may override via `limits({ maxBytes, maxPixels, timeoutMs })`; `0` disables a limit.
- Firewall errors are surfaced as `ResourceLimit / FirewallViolation` with stage context (decode/process/write).

## Concurrency
- `processBatch` concurrency: 0 = auto, 1–1024 allowed; >1024 rejected.
- Streaming pipeline uses a single encode worker; throughput can be scaled by running multiple pipelines.

## Memory expectations
- Zero-copy input path (fromPath/processBatch) avoids copying source data into JS heap.
- Target RSS budget: `peak_rss ≤ decoded_bytes + 24 MB` (see spec/quality.md for measurement references).
