# Metrics API (v1.1.0)

lazy-image exposes structured telemetry via `ImageEngine.toBufferWithMetrics()`, `ImageEngine.toFileWithMetrics()`, and `ImageEngine.processBatchWithMetrics()`. The single-image payload is versioned and validated so that downstream services can ingest it safely.

## Payload shape

```jsonc
{
  "version": "1.1.0",
  "decodeMs": 12.4,
  "opsMs": 8.1,
  "encodeMs": 15.3,
  "totalMs": 37.0,
  "peakRss": 28450000,
  "cpuTime": 0.024,
  "processingTime": 0.037,
  "bytesIn": 152004,
  "bytesOut": 62488,
  "compressionRatio": 0.41,
  "formatIn": "jpeg",
  "formatOut": "webp",
  "iccPreserved": false,
  "iccOutcome": "absent",
  "metadataStripped": true,
  "policyViolations": ["firewall_rejected_metadata"]
}
```

## Field guide

- **version**: Schema version. Current value: `1.1.0`.
- **decodeMs / opsMs / encodeMs / totalMs**: Wall-clock timings in milliseconds.
- **peakRss**: Process RSS high-water mark or an estimate, in bytes; see measurement boundaries below.
- **cpuTime**: CPU time (user + system) in seconds.
- **bytesIn / bytesOut / compressionRatio**: I/O sizes and ratio (`bytesOut / bytesIn`).
- **formatIn / formatOut**: Detected input format (nullable) and requested output format.
- **iccPreserved / metadataStripped**: Whether ICC profile was preserved or metadata was stripped.
- **iccOutcome**: `absent`, `preserved`, `unsafe-stripped`, `policy-stripped`, or `unsupported`; use this instead of inferring ICC presence from `iccPreserved: false`.
- **policyViolations**: Non-fatal Image Firewall actions that altered output (e.g., forced metadata strip under strict policy).

## Validation

- Formal JSON Schema: `docs/metrics-schema.json` (Draft 2020-12). Use this for contract tests or ingestion validation.
- TypeScript types are emitted in `index.d.ts` under `ProcessingMetrics`.
- Batch APIs add wrapper types (`FileOutputWithMetrics`, `BatchResultWithMetrics`, `BatchMetricsSummary`, `BatchOutputWithMetrics`) around the same `ProcessingMetrics` item payload.

## Stability policy

- Additive changes only within minor versions. Breaking changes (field removal/rename or semantic shifts) require a new `version` value.
- Downstream clients should gate on `version` and ignore unknown fields to remain forward compatible.
- The v1.x contract contains only the canonical fields above. Legacy metric
  aliases removed before v1 are not emitted and are not accepted by the schema.

## Measurement boundaries

`decodeMs`, `opsMs` and `encodeMs` describe decode, queued operations and encoding.
`totalMs` is elapsed pipeline time; `processingTime` is `totalMs / 1000` in seconds.
These are not upload latency, queue wait time or complete compiler publication time.

On Linux/macOS, `peakRss` uses `ru_maxrss`: the maximum for the process lifetime,
including earlier jobs. It cannot isolate one image's allocation. When resource
usage is unavailable, the processing path estimates decoded pixels plus encoded
output size and leaves CPU time at zero. RSS and byte counts are clamped to `u32`;
CPU time is a non-negative process usage delta when available. Compare isolated
processes when evaluating workloads; do not label this field encoder-only memory.

The formal schema is [metrics-schema.json](./metrics-schema.json); a logging
example lives in [examples](../examples/metrics-observability.mjs).
