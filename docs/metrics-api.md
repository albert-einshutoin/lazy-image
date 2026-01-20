# Metrics API (v1.0.0)

lazy-image exposes structured telemetry via `ImageEngine.toBufferWithMetrics()`. The payload is versioned and validated so that downstream services can ingest it safely.

## Payload shape

```jsonc
{
  "version": "1.0.0",
  "decodeMs": 12.4,
  "opsMs": 8.1,
  "encodeMs": 15.3,
  "totalMs": 37.0,
  "peakRss": 28_450_000,
  "cpuTime": 0.024,
  "processingTime": 0.037,
  "bytesIn": 152004,
  "bytesOut": 62488,
  "compressionRatio": 0.41,
  "formatIn": "jpeg",
  "formatOut": "webp",
  "iccPreserved": false,
  "metadataStripped": true,
  "policyViolations": ["firewall_rejected_metadata"],
  // Legacy aliases kept for backward compatibility
  "decodeTime": 12.4,
  "processTime": 8.1,
  "encodeTime": 15.3,
  "memoryPeak": 28450000,
  "inputSize": 152004,
  "outputSize": 62488
}
```

## Field guide

- **version**: Schema version. Current value: `1.0.0`.
- **decodeMs / opsMs / encodeMs / totalMs**: Wall-clock timings in milliseconds.
- **peakRss**: Peak resident set size (bytes) during the operation.
- **cpuTime**: CPU time (user + system) in seconds.
- **bytesIn / bytesOut / compressionRatio**: I/O sizes and ratio (`bytesOut / bytesIn`).
- **formatIn / formatOut**: Detected input format (nullable) and requested output format.
- **iccPreserved / metadataStripped**: Whether ICC profile was preserved or stripped.
- **policyViolations**: Non-fatal Image Firewall actions that altered output (e.g., forced metadata strip under strict policy).
- **Legacy aliases**: `decodeTime`, `processTime`, `encodeTime`, `memoryPeak`, `inputSize`, `outputSize` map 1:1 to the new fields for compatibility.

## Validation

- Formal JSON Schema: `docs/metrics-schema.json` (Draft 2020-12). Use this for contract tests or ingestion validation.
- TypeScript types are emitted in `index.d.ts` under `ProcessingMetrics`.

## Stability policy

- Additive changes only within minor versions. Breaking changes (field removal/rename or semantic shifts) require a new `version` value.
- Downstream clients should gate on `version` and ignore unknown fields to remain forward compatible.
