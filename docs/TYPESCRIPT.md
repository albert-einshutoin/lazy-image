# TypeScript Usage Guide

lazy-image ships comprehensive TypeScript definitions generated from NAPI-RS and augmented with stricter types for the JS helper layer.

## Setup

```bash
npm install @alberteinshutoin/lazy-image
```

Types are included — no separate `@types/` package needed.

## Importing

```typescript
import {
  ImageEngine,
  ErrorCategory,
  ErrorCode,
  getErrorCategory,
  inspectFile,
  inspect,
  createStreamingPipeline,
  resolveEncodeProfile,
} from '@alberteinshutoin/lazy-image';

// Types are also importable
import type {
  OutputFormat,
  InputFormat,
  PresetName,
  ResizeFit,
  ProcessingMetrics,
  BatchResult,
  EncodeOptionsInput,
  TargetBytesOptions,
} from '@alberteinshutoin/lazy-image';
```

## Type-Safe Error Handling

lazy-image uses a 4-tier `ErrorCategory` enum. The `getErrorCategory()` helper extracts it from any thrown error:

```typescript
import { ImageEngine, ErrorCategory, getErrorCategory } from '@alberteinshutoin/lazy-image';

try {
  await ImageEngine.fromPath('input.jpg')
    .resize(800)
    .toBuffer('jpeg', 85);
} catch (err: unknown) {
  const category = getErrorCategory(err);

  switch (category) {
    case ErrorCategory.UserError:
      // Invalid input — caller can fix (e.g., bad rotation angle, file not found)
      break;
    case ErrorCategory.CodecError:
      // Format/encoding issue — may need different format or re-encoded source
      break;
    case ErrorCategory.ResourceLimit:
      // Dimension/memory limit — resize or reject the input
      break;
    case ErrorCategory.InternalBug:
      // Library bug — please report at github.com/albert-einshutoin/lazy-image/issues
      break;
    case null:
      // Not a lazy-image error (no error code set)
      break;
  }
}
```

Thrown errors also carry fine-grained properties:

```typescript
try {
  // ...
} catch (err: unknown) {
  if (err instanceof Error) {
    const e = err as Error & { errorCode?: string; recoveryHint?: string };
    console.log(e.errorCode);    // e.g. "E200"
    console.log(e.recoveryHint); // e.g. "Ensure crop coordinates are within image bounds"
  }
}
```

## Case-Insensitive Format Types

`OutputFormat`, `InputFormat`, and `PresetName` accept case variants:

```typescript
// All valid — the CaseInsensitive<T> utility type allows these
const buf1 = await engine.toBuffer('jpeg', 85);
const buf2 = await engine.toBuffer('JPEG', 85);
const buf3 = await engine.toBuffer('Jpeg', 85);
```

## Batch Processing Types

`processBatch()` returns `BatchResult[]` with typed error metadata:

```typescript
const results: BatchResult[] = await engine.processBatch(files, outDir, {
  format: 'webp',
  quality: 80,
});

for (const r of results) {
  if (r.success) {
    console.log(r.outputPath); // string
  } else {
    console.log(r.error);         // string — human-readable message
    console.log(r.errorCode);     // string — e.g. "E100"
    console.log(r.errorCategory); // ErrorCategory enum value
  }
}
```

## Encode Options (Unified API)

The `encode()` and `encodeToFile()` methods accept a typed options object:

```typescript
const { data, metrics } = await engine.encode({
  format: 'webp',
  quality: 80,
  metrics: true,
});

// Preset-based encoding
const result = await engine.encode({ preset: 'hero', metrics: true });
```

## Target-Bytes Encoding

```typescript
import type { TargetBytesOptions, BufferTargetBytesResult } from '@alberteinshutoin/lazy-image';

const result: BufferTargetBytesResult = await engine.toBufferTargetBytes('jpeg', {
  targetBytes: 100_000,
  minQuality: 30,
  maxQuality: 95,
  qualityFloorPolicy: 'best-effort',
});

if (result.budgetMet) {
  console.log(`Met budget at quality ${result.quality}`);
}
```

## Processing Metrics

```typescript
import type { ProcessingMetrics } from '@alberteinshutoin/lazy-image';

const { data, metrics } = await engine.toBufferWithMetrics('jpeg', 85);

// All fields are typed
console.log(metrics.decodeMs);          // number
console.log(metrics.encodeMs);          // number
console.log(metrics.compressionRatio);  // number
console.log(metrics.iccPreserved);      // boolean
console.log(metrics.formatIn);          // InputFormat | undefined
console.log(metrics.formatOut);         // CanonicalOutputFormat
```

## IDE Tips

- **Auto-complete**: All methods on `ImageEngine` are fully typed with JSDoc descriptions.
- **Hover docs**: Method signatures include parameter descriptions, defaults, and usage notes.
- **Error narrowing**: Use `getErrorCategory()` for exhaustive switch/case — TypeScript will warn on unhandled categories.

## Further Reading

- [Full API reference](./API.md)
- [Error codes](./ERROR_CODES.md)
- [Examples](../examples/)
